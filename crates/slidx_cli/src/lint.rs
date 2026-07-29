//! `slidx lint` — the linter, on a deck on disk.
//!
//! The same rules the plugin runs during `vite build` and the editor runs while
//! the author types, reached from a terminal. Nothing is re-implemented here:
//! the deck is parsed by [`slidx_core`], the colours come from
//! [`slidx_theme`]'s resolved surfaces, and the findings come from
//! [`slidx_lint`]. This module reads a path and prints.
//!
//! ## The exit code is the whole point
//!
//! `1` when something blocking is found, `0` otherwise. That one number is what
//! makes the linter usable in somebody's CI, and it is the reason this command
//! exists at all rather than being left to the plugin — a check that only runs
//! inside a build is a check that only runs where a build runs.
//!
//! Blocking means severity `error`: content that was dropped, or an asset the
//! deck fetches from the network. The offline guarantee is the one slidx makes
//! out loud, so breaking it fails the run rather than warning about it.
//!
//! A deck that could not be read exits `2`, never `1` and never `0` — see the
//! crate docs. A CI job that mistyped a path has to fail differently from one
//! whose deck has a problem.
//!
//! ## What this cannot see
//!
//! Rules that need a laid-out page — content overflow, safe-area bleed — do not
//! run here, because nothing has been rendered. They run in the build, where
//! there is a page to measure. This command is the subset that is decidable
//! from the source and the theme, which is most of them.

pub mod source;

use std::path::{Path, PathBuf};

use slidx_core::{parse_deck, Deck, DeckParseOptions, Diagnostic, Severity};
use slidx_lint::{lint, LintInput, LintOptions};

use crate::args::Matches;
use crate::home::Home;
use crate::index::{self, Entry};
use crate::report;
use crate::style::{Ink, Style};
use crate::{Outcome, FOUND, OK};

/// Past this a slide title is cut short in a finding's locator.
///
/// A cap rather than a target: most locators are the same width, so the eye
/// learns where the rule code starts. The title is there to help somebody find
/// the slide and the number already does that, so it is the part that gives way
/// when the line runs out of room. Content is never truncated — only this.
const TITLE_BUDGET: usize = 32;

/// Below this there is no room for a title worth reading, so it is dropped
/// rather than printed as a stub.
const TITLE_FLOOR: usize = 8;

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let separator =
        matches.value("separator").map(str::to_string).unwrap_or_else(|| "---".to_string());

    let path = matches
        .first_positional()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(source::DEFAULT_DIR));

    let deck_source = match source::read(&path, &separator) {
        Ok(deck_source) => deck_source,
        Err(message) => return Outcome::misuse(format!("{message}\n")),
    };

    let deck = parse_deck(
        &deck_source.source,
        &DeckParseOptions { separator, ..DeckParseOptions::default() },
    );

    // The index fills itself: running a command on a deck is what puts it in
    // the list, so nobody has to remember to register anything. Best-effort in
    // the strongest sense — see `index::remember`.
    if let Some(root) = project_root(&path) {
        index::remember(&Home::discover().index(), Entry::new(root).describing(&deck));
    }

    let diagnostics = collect(&deck, matches);

    let text = if matches.is_set("json") {
        match serde_json::to_string_pretty(&diagnostics) {
            Ok(json) => format!("{json}\n"),
            Err(error) => {
                return Outcome::misuse(format!("could not serialise the findings: {error}\n"))
            }
        }
    } else {
        render(&deck, &diagnostics, &deck_source.label, style)
    };

    Outcome::out(text).with_code(exit_code(&diagnostics))
}

/// The directory the index should remember, given the path that was linted.
///
/// The *project*, not the slides folder inside it: `slidx lint ./slides` in
/// `~/talks/vueconf` should record `~/talks/vueconf`, because that is the
/// directory somebody wants to open a year later — the one with the git
/// repository, the vite config and the deck in it.
///
/// Absolute, so an entry recorded from one working directory still means
/// something read from another. A path that will not canonicalise is not
/// recorded at all rather than stored as a relative fragment that resolves to
/// somewhere else later.
fn project_root(linted: &Path) -> Option<PathBuf> {
    let full = linted.canonicalize().ok()?;
    let directory = if full.is_dir() { full } else { full.parent()?.to_path_buf() };

    // `./slides` is the conventional layout, so its parent is the project. A
    // deck kept anywhere else is its own project.
    match directory.file_name().and_then(|name| name.to_str()) {
        Some(source::DEFAULT_DIR) => directory.parent().map(Path::to_path_buf),
        _ => Some(directory),
    }
}

/// Parse diagnostics and lint findings, in that order.
///
/// The same order and the same set the wasm pipeline reports, so a deck that is
/// clean here is a deck that builds clean. A parse problem comes first because
/// it explains the lint findings underneath it: a slide that failed to parse
/// lints as an empty slide.
fn collect(deck: &Deck, matches: &Matches) -> Vec<Diagnostic> {
    let theme = resolve_theme(matches.value("theme"), deck.meta.theme.as_deref());
    let surfaces = theme.surfaces();

    let options = LintOptions {
        allow: matches.values("allow").map(str::to_string).collect(),
        strict: matches.is_set("strict"),
        ..LintOptions::default()
    };

    let mut diagnostics: Vec<Diagnostic> = deck.diagnostics.iter().cloned().collect();
    diagnostics.extend(lint(&LintInput::new(deck, &surfaces), &options));

    // Worst first, then in deck order. The plugin emits findings in rule order,
    // which is right for a build log interleaved with everything else Vite is
    // saying; a report read on its own has to put the thing that fails the run
    // at the top, and then let somebody walk the deck once per severity.
    //
    // A stable sort, so two findings on the same slide at the same severity
    // keep the order the rules produced them in rather than swapping between
    // runs for no visible reason.
    diagnostics.sort_by_key(|diagnostic| {
        (std::cmp::Reverse(diagnostic.severity), diagnostic.span.slide_index)
    });

    diagnostics
}

/// The theme the deck will actually be rendered with.
///
/// The same fallback the wasm pipeline uses — the flag, then the deck's own
/// `theme:`, then the default. Resolving differently here would lint one
/// deck's colours and ship another's.
fn resolve_theme(requested: Option<&str>, from_deck: Option<&str>) -> slidx_theme::Theme {
    requested
        .or(from_deck)
        .and_then(slidx_theme::resolve)
        .unwrap_or_else(slidx_theme::default_theme)
}

fn exit_code(diagnostics: &[Diagnostic]) -> u8 {
    if diagnostics.iter().any(Diagnostic::is_blocking) {
        FOUND
    } else {
        OK
    }
}

/// The report, as a person reads it.
pub fn render(deck: &Deck, diagnostics: &[Diagnostic], label: &str, style: &Style) -> String {
    let titles: Vec<Option<&str>> =
        deck.slides.iter().map(|slide| slide.title.as_deref()).collect();

    let mut text = format!(
        "{} {}\n\n  {}\n",
        style.paint(Ink::Strong, "slidx lint"),
        style.paint(Ink::Faint, label),
        verdict(deck, diagnostics, style)
    );

    for diagnostic in diagnostics {
        text.push('\n');
        text.push_str(&report::block(
            diagnostic.severity.as_token(),
            ink_for(diagnostic.severity),
            &subject(diagnostic, &titles),
            &diagnostic.message,
            diagnostic.help.as_deref(),
            style,
        ));
    }

    text
}

/// Where a finding is, and which rule found it.
///
/// The slide number leads because it is what a person acts on — they open that
/// slide. The rule code trails, for looking up or suppressing. The wording of
/// the message and the help underneath is the diagnostic's own, so a finding
/// hit in `vite build` and again here says the same sentence in both places.
fn subject(diagnostic: &Diagnostic, titles: &[Option<&str>]) -> String {
    let Some(index) = diagnostic.span.slide_index else {
        return format!("deck  [{}]", diagnostic.code);
    };

    let tail = format!("  [{}]", diagnostic.code);
    let head = format!("slide {}", index + 1);

    // What is left on the line once the number and the code have had their
    // room. The title gives way, not the code — a finding you cannot look up is
    // worse than one whose slide you have to count to.
    let room = crate::style::WIDTH
        .saturating_sub(report::INDENT + head.chars().count() + tail.chars().count() + 3);

    match titles.get(index as usize).copied().flatten() {
        Some(title) if room >= TITLE_FLOOR => {
            format!("{head} ({}){tail}", shorten(title, room.min(TITLE_BUDGET)))
        }
        _ => format!("{head}{tail}"),
    }
}

/// Cuts a slide title down to a locator. See [`TITLE_BUDGET`].
fn shorten(title: &str, budget: usize) -> String {
    if title.chars().count() <= budget {
        return title.to_string();
    }

    // Cut on a character boundary and mark the cut, so nobody reads the short
    // form as the actual title and then fails to find it in the deck.
    format!("{}...", title.chars().take(budget - 3).collect::<String>().trim_end())
}

/// The one line somebody reads if they read only one.
fn verdict(deck: &Deck, diagnostics: &[Diagnostic], style: &Style) -> String {
    let blocking = diagnostics.iter().filter(|d| d.is_blocking()).count();
    let rest = diagnostics.len() - blocking;
    let slides = slides(deck.slides.len());

    if diagnostics.is_empty() {
        return style.paint(Ink::Pass, format!("Nothing to fix. {slides}, all clear."));
    }

    let mut said = Vec::new();
    if blocking > 0 {
        said.push(style.paint(Ink::Fail, format!("{blocking} blocking")));
    }
    if rest > 0 {
        said.push(style.paint(Ink::Warn, format!("{rest} worth a look")));
    }

    format!("{} across {slides}.", said.join(", "))
}

/// "1 slide" rather than "1 slides".
///
/// Small, and worth the three lines: a tool that cannot count its own nouns
/// reads as one that was not finished, and this line is the first thing anybody
/// sees.
fn slides(count: usize) -> String {
    format!("{count} slide{}", if count == 1 { "" } else { "s" })
}

fn ink_for(severity: Severity) -> Ink {
    match severity {
        Severity::Error => Ink::Fail,
        Severity::Warning => Ink::Warn,
        Severity::Info => Ink::Faint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::SourceSpan;

    fn matches_for(line: &str) -> Matches {
        let argv: Vec<String> =
            format!("lint {line}").split_whitespace().map(String::from).collect();

        match crate::args::parse(&argv) {
            crate::args::Invocation::Run(_, matches) => matches,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    fn parse(source: &str) -> Deck {
        parse_deck(source, &DeckParseOptions::default())
    }

    /// A deck that fetches an image over the network — the one guarantee slidx
    /// makes out loud, and so the one rule that blocks.
    const REMOTE_ASSET: &str = "# One\n\n![a diagram](https://cdn.example.com/a.png)\n";

    #[test]
    fn a_deck_that_reaches_the_network_for_an_asset_exits_non_zero() {
        // The reason this command exists rather than being left to the build:
        // somebody's CI has to be able to fail on it.
        let deck = parse(REMOTE_ASSET);
        let diagnostics = collect(&deck, &matches_for(""));

        assert!(diagnostics.iter().any(|d| d.code.starts_with("offline/")), "{diagnostics:?}");
        assert_eq!(exit_code(&diagnostics), FOUND);
    }

    #[test]
    fn findings_that_are_only_advice_do_not_fail_the_run() {
        // A CI job that went red over a missing alt attribute would be turned
        // off within a week, and then nothing would be checked at all.
        let deck = parse("# One\n\n![](./a.png)\n");
        let diagnostics = collect(&deck, &matches_for(""));

        assert!(diagnostics.iter().any(|d| d.code == "structure/missing-alt"), "{diagnostics:?}");
        assert_eq!(exit_code(&diagnostics), OK);
    }

    #[test]
    fn a_clean_deck_reports_nothing_and_exits_zero() {
        let deck = parse("# One\n\n- a\n- b\n");
        let diagnostics = collect(&deck, &matches_for(""));

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(exit_code(&diagnostics), OK);
    }

    #[test]
    fn allow_suppresses_a_rule_and_a_whole_group_alike() {
        let deck = parse(REMOTE_ASSET);

        assert!(collect(&deck, &matches_for("--allow offline")).is_empty());
        assert!(collect(&deck, &matches_for("--allow offline/remote-asset"))
            .iter()
            .all(|d| d.code != "offline/remote-asset"));
    }

    #[test]
    fn allow_can_be_given_more_than_once() {
        let deck = parse("# One\n\n![](./a.png)\n\n---\n\n### Skipped\n");
        let suppressed = collect(&deck, &matches_for("--allow structure --allow contrast"));

        assert!(suppressed.iter().all(|d| !d.code.starts_with("structure/")), "{suppressed:?}");
    }

    #[test]
    fn parse_diagnostics_are_reported_before_the_lint_findings() {
        // A slide that failed to parse lints as an empty slide, so the parse
        // problem has to be the first thing read — it explains the rest.
        let deck = parse("---\naspect: nonsense\n---\n\n# One\n\n![](./a.png)\n");
        let diagnostics = collect(&deck, &matches_for(""));

        let codes: Vec<&str> = diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, ["deck/unknown-aspect", "structure/missing-alt"], "{diagnostics:?}");
    }

    #[test]
    fn the_theme_flag_decides_which_colours_are_checked() {
        // Linting one theme's colours and shipping another's would be worse
        // than not linting at all: it would be a green run about a deck nobody
        // is going to show.
        let requested = resolve_theme(Some("terminal"), Some("minimal"));
        let from_deck = resolve_theme(None, Some("terminal"));

        assert_eq!(requested.id, "terminal");
        assert_eq!(from_deck.id, "terminal");
    }

    #[test]
    fn an_unknown_theme_name_falls_back_rather_than_failing_the_run() {
        // A typo in `theme:` is already reported by the parser. Refusing to
        // lint on top of that would hide every other finding behind it.
        assert_eq!(resolve_theme(Some("no-such-theme"), None).id, slidx_theme::default_theme().id);
    }

    #[test]
    fn the_verdict_is_the_first_thing_under_the_title() {
        let deck = parse(REMOTE_ASSET);
        let diagnostics = collect(&deck, &matches_for(""));
        let text = render(&deck, &diagnostics, "slides", &Style::plain());
        let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();

        assert!(lines[1].contains("1 blocking"), "{text}");
    }

    #[test]
    fn a_clean_deck_is_told_so_in_a_sentence_rather_than_by_an_empty_report() {
        let deck = parse("# One\n");
        let text = render(&deck, &[], "slides", &Style::plain());

        assert!(text.contains("Nothing to fix"), "{text}");
    }

    #[test]
    fn a_finding_names_the_slide_by_number_and_title() {
        // The number is what a person acts on: they open that slide. Counting
        // from one, because nobody counts slides from zero out loud.
        let deck = parse("# First\n\n---\n\n# Second\n");
        let diagnostic =
            Diagnostic::warning("test/example", "something").at(SourceSpan::line(3).on_slide(1));

        let text = render(&deck, std::slice::from_ref(&diagnostic), "slides", &Style::plain());

        assert!(text.contains("slide 2 (Second)"), "{text}");
    }

    #[test]
    fn a_deck_wide_finding_says_deck_rather_than_naming_a_slide() {
        let deck = parse("# One\n");
        let diagnostic = Diagnostic::warning("budget/over", "the deck runs long");
        let text = render(&deck, std::slice::from_ref(&diagnostic), "slides", &Style::plain());

        assert!(text.contains("deck  [budget/over]"), "{text}");
        assert!(!text.contains("slide 1"), "{text}");
    }

    #[test]
    fn the_verdict_counts_one_slide_as_a_slide_rather_than_as_slides() {
        // A tool that cannot count its own nouns reads as one that was not
        // finished, and this is the first line anybody sees.
        assert!(render(&parse("# One\n"), &[], "slides", &Style::plain()).contains("1 slide,"));
        assert!(render(&parse("# One\n\n---\n\n# Two\n"), &[], "slides", &Style::plain())
            .contains("2 slides,"));
    }

    #[test]
    fn a_long_slide_title_is_cut_short_in_the_locator_and_marked_as_cut() {
        // The title is there to help somebody find the slide; the number
        // already does that. One long enough to push the rule code onto a
        // second line costs more than it gives.
        let deck = parse("# A heading long enough that it would run off the end of the line\n");
        let diagnostic =
            Diagnostic::warning("test/example", "x").at(SourceSpan::line(1).on_slide(0));
        let text = render(&deck, std::slice::from_ref(&diagnostic), "slides", &Style::plain());

        assert!(text.contains("..."), "{text}");
        for line in text.lines() {
            assert!(line.chars().count() <= crate::style::WIDTH, "{line}");
        }
    }

    #[test]
    fn a_locator_gives_up_title_before_it_gives_up_the_rule_code() {
        // A finding you cannot look up or suppress is worse than one whose
        // slide you have to count to. So when the line runs out of room it is
        // the title that shrinks, and the code that stays whole.
        let deck = parse("# A heading long enough that it would run off the end of the line\n");
        let diagnostic = Diagnostic::warning("structure/an-extremely-long-rule-code-indeed", "x")
            .at(SourceSpan::line(1).on_slide(0));
        let text = render(&deck, std::slice::from_ref(&diagnostic), "slides", &Style::plain());

        assert!(text.contains("[structure/an-extremely-long-rule-code-indeed]"), "{text}");
        for line in text.lines() {
            assert!(
                line.chars().count() <= crate::style::WIDTH,
                "{} cols: {line}",
                line.chars().count()
            );
        }
    }

    #[test]
    fn a_blocking_finding_is_printed_before_the_advice() {
        // The one that fails the run has to be the first thing read. The plugin
        // emits in rule order, which is right for a build log and wrong for a
        // report somebody is reading on its own.
        let deck = parse("# One\n\n![](./a.png)\n\n![b](https://cdn.example.com/b.png)\n");
        let diagnostics = collect(&deck, &matches_for(""));

        assert_eq!(diagnostics[0].severity, Severity::Error, "{diagnostics:?}");
    }

    #[test]
    fn findings_of_equal_severity_are_printed_in_deck_order() {
        // So an author can walk the deck once per severity rather than jumping
        // back and forth between slides.
        let deck = parse("# One\n\n![](./a.png)\n\n---\n\n# Two\n\n![](./b.png)\n");
        let slides: Vec<Option<u32>> =
            collect(&deck, &matches_for("")).iter().map(|d| d.span.slide_index).collect();

        let mut sorted = slides.clone();
        sorted.sort_unstable();
        assert_eq!(slides, sorted, "{slides:?}");
    }

    #[test]
    fn a_findings_help_is_printed_under_it_rather_than_dropped() {
        // The help is the only actionable half of a diagnostic.
        let deck = parse("# One\n");
        let diagnostic = Diagnostic::warning("test/example", "something").with_help("Do this.");

        assert!(render(&deck, std::slice::from_ref(&diagnostic), "slides", &Style::plain())
            .contains("Do this."));
    }

    #[test]
    fn a_finding_carries_its_code_for_looking_up_or_suppressing() {
        let deck = parse("# One\n");
        let diagnostic = Diagnostic::warning("contrast/projector", "washed out");

        assert!(render(&deck, std::slice::from_ref(&diagnostic), "slides", &Style::plain())
            .contains("[contrast/projector]"));
    }

    #[test]
    fn the_project_recorded_is_the_directory_above_a_conventional_slides_folder() {
        // `slidx lint ./slides` in ~/talks/vueconf has to remember
        // ~/talks/vueconf. That is the directory with the git repository and
        // the vite config in it — the one somebody wants to open again.
        let scratch = std::env::temp_dir().join(format!("slidx-root-{}", std::process::id()));
        let slides = scratch.join("slides");
        std::fs::create_dir_all(&slides).expect("scratch");

        let root = project_root(&slides).expect("a project");
        assert_eq!(root.canonicalize().ok(), scratch.canonicalize().ok());

        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn a_deck_kept_somewhere_other_than_slides_is_its_own_project() {
        let scratch = std::env::temp_dir().join(format!("slidx-root-own-{}", std::process::id()));
        std::fs::create_dir_all(&scratch).expect("scratch");

        assert_eq!(
            project_root(&scratch).and_then(|path| path.canonicalize().ok()),
            scratch.canonicalize().ok()
        );

        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn a_single_file_deck_records_the_directory_it_sits_in() {
        let scratch = std::env::temp_dir().join(format!("slidx-root-file-{}", std::process::id()));
        std::fs::create_dir_all(&scratch).expect("scratch");
        let file = scratch.join("talk.md");
        std::fs::write(&file, "# One\n").expect("write");

        assert_eq!(
            project_root(&file).and_then(|path| path.canonicalize().ok()),
            scratch.canonicalize().ok()
        );

        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn a_path_that_does_not_exist_is_not_recorded_at_all() {
        // Rather than stored as a relative fragment that would resolve to
        // somewhere else entirely when it is read back.
        assert!(project_root(Path::new("/nowhere/at/all")).is_none());
    }

    #[test]
    fn a_missing_deck_exits_two_rather_than_reporting_a_clean_run() {
        // The failure that matters most in CI: a mistyped path must not look
        // like a deck with nothing wrong with it.
        let outcome = run(&matches_for("/nowhere/at/all"), &Style::plain());

        assert_eq!(outcome.code, crate::MISUSE);
        assert!(outcome.stdout.is_empty());
        assert!(outcome.stderr.contains("/nowhere/at/all"), "{}", outcome.stderr);
    }

    #[test]
    fn a_report_lines_up_the_same_coloured_and_plain() {
        let deck = parse(REMOTE_ASSET);
        let diagnostics = collect(&deck, &matches_for(""));

        let plain = render(&deck, &diagnostics, "slides", &Style::plain());
        let colored = render(&deck, &diagnostics, "slides", &Style::colored());

        assert_eq!(plain.lines().count(), colored.lines().count());
        assert!(!plain.contains('\u{1b}'));
    }
}
