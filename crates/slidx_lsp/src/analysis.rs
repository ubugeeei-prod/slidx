//! One pass over a document, and everything the other features read from it.
//!
//! # What an edit costs
//!
//! A whole re-parse. Every stage of the slidx pipeline is a pure function of
//! its input, so a *correct* incremental parser is possible, but it is not
//! what makes an editor feel slow: an eighty-slide deck is tens of kilobytes
//! and a full analysis of one is well under a millisecond. What makes an
//! editor feel slow is doing that work once per keystroke while the keystrokes
//! are still arriving.
//!
//! So the saving is made one level up. An edit marks a document dirty and
//! does no work; the analysis happens when the server has drained its input
//! queue, which is once per burst of typing rather than once per character.
//! See [`crate::server::Server::flush`]. Should a deck ever grow past the
//! point where that is enough, the next move is per-segment memoisation —
//! [`slidx_core::parser::split`] already isolates the slide that changed.
//!
//! # An open fence must not blank the outline
//!
//! Parsing a deck never fails, so there is no error path that loses the
//! symbol tree. There is something subtler. `---` inside a fenced code block
//! is content rather than a slide break, so the moment an author types the
//! opening ``` of a fence, every separator below it is swallowed and eighty
//! slides collapse into one. The outline empties and the editor's breadcrumbs
//! and jump-to-slide go with it, until the closing fence is typed.
//!
//! That state is precisely detectable — the document ends inside a fence — so
//! this module reports it rather than guessing, and [`crate::symbols`] serves
//! the last trustworthy outline while it holds.

use slidx_core::parser::{split, Segment};
use slidx_core::{parse_deck, scanner, Deck, DeckParseOptions, Diagnostics};
use slidx_lint::{lint, LintInput, LintOptions};

use crate::position::LineIndex;

/// A run of one-based, inclusive source lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineSpan {
    pub first: u32,
    pub last: u32,
}

impl LineSpan {
    pub fn contains(&self, line: u32) -> bool {
        line >= self.first && line <= self.last
    }
}

/// A frontmatter block, located in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrontmatterBlock {
    /// Index of the slide it configures.
    pub slide: usize,
    pub lines: LineSpan,
}

impl FrontmatterBlock {
    /// True when this block is also the deck's own configuration.
    ///
    /// The first slide's frontmatter is the deck's, which is why `title:` and
    /// `theme:` are offered there and nowhere else.
    pub fn is_deck(&self) -> bool {
        self.slide == 0
    }
}

/// Everything one pass over a document produced.
#[derive(Debug, Clone)]
pub struct Analysis {
    pub deck: Deck,
    /// Parse diagnostics and lint findings, in that order.
    pub findings: Diagnostics,
    /// Source extent of each slide, parallel to `deck.slides`.
    pub slides: Vec<LineSpan>,
    pub frontmatter: Vec<FrontmatterBlock>,
    /// Set while the author is typing the deck's opening frontmatter and has
    /// not closed it yet, which is a block no parser can see as one.
    pub unclosed_frontmatter: Option<LineSpan>,
    /// True when the document ends inside a fenced code block.
    pub open_fence: bool,
}

impl Analysis {
    /// False while a half-typed fence is swallowing the separators below it.
    pub fn outline_is_trustworthy(&self) -> bool {
        !self.open_fence
    }

    /// The slide a one-based source line belongs to.
    pub fn slide_at(&self, line: u32) -> Option<usize> {
        self.slides.iter().position(|span| span.contains(line))
    }

    /// The first line of a slide with anything on it, one-based.
    ///
    /// Where a diagnostic about the slide is underlined, and where selecting
    /// the slide in an outline lands. A slide's own first line is usually the
    /// separator above it or a blank, and pointing at one of those reads as a
    /// tool that has lost its place.
    pub fn content_line(&self, at: usize, text: &str, index: &LineIndex) -> u32 {
        let (Some(extent), Some(slide)) = (self.slides.get(at), self.deck.slides.get(at)) else {
            return 1;
        };

        (slide.source_line..=extent.last)
            .find(|line| !index.line_text(text, line.saturating_sub(1)).trim().is_empty())
            .unwrap_or(slide.source_line)
            .clamp(extent.first, extent.last)
    }

    /// The frontmatter block a one-based source line sits inside, including
    /// one the author has opened and not yet closed.
    pub fn frontmatter_at(&self, line: u32) -> Option<FrontmatterBlock> {
        if let Some(lines) = self.unclosed_frontmatter.filter(|span| span.contains(line)) {
            return Some(FrontmatterBlock { slide: 0, lines });
        }

        self.frontmatter.iter().copied().find(|block| block.lines.contains(line))
    }
}

/// Parses and lints a document.
pub fn analyze(source: &str) -> Analysis {
    let options = DeckParseOptions::default();
    let deck = parse_deck(source, &options);
    let segments = split(source, &options.separator);

    let total_lines = source.lines().count().max(1) as u32;
    let starts = slide_starts(&segments);

    let slides = starts
        .iter()
        .enumerate()
        .map(|(index, first)| LineSpan {
            first: *first,
            last: starts.get(index + 1).map_or(total_lines, |next| next.saturating_sub(1)),
        })
        .collect();

    // Parse, then dialect, then room. The dialect findings come second because
    // they explain the ones under them: a `theme:` nobody can resolve is why the
    // contrast findings below it are the default theme's.
    let mut findings = deck.diagnostics.clone();
    findings.extend(slidx_dialect::check(&deck, &[]));
    findings.extend(lint_deck(&deck));

    Analysis {
        slides,
        frontmatter: frontmatter_blocks(&segments),
        unclosed_frontmatter: unclosed_frontmatter(source, &options.separator, total_lines),
        open_fence: ends_inside_a_fence(source),
        deck,
        findings,
    }
}

/// Runs the linter against the theme the deck actually names.
///
/// Resolving the theme is what makes contrast and font-size findings real: a
/// deck on `terminal` and one on `contrast` fail different rules, and linting
/// both against the default would report the wrong answer for one of them.
///
/// An unknown name falls back to the default here rather than being reported
/// here. It is reported — `slidx_dialect` says so, above — and this crate still
/// invents nothing: the finding carries that crate's code and remedy, the same
/// ones `slidx lint` prints.
fn lint_deck(deck: &Deck) -> Diagnostics {
    let theme = deck
        .meta
        .theme
        .as_deref()
        .and_then(slidx_theme::resolve)
        .unwrap_or_else(slidx_theme::default_theme);

    lint(&LintInput::new(deck, &theme.surfaces()), &LintOptions::default())
}

/// The first line of each slide, one-based.
///
/// A separator ends the slide above it, so a slide begins on the line after —
/// or at its own frontmatter, whose first key is the first thing the author
/// wrote for it. The first slide always starts at line one so that deck
/// frontmatter belongs to something.
fn slide_starts(segments: &[Segment]) -> Vec<u32> {
    segments
        .iter()
        .enumerate()
        .map(|(index, segment)| match (index, &segment.frontmatter) {
            (0, _) => 1,
            (_, Some(matter)) => matter.line,
            (_, None) => segment.line,
        })
        .collect()
}

fn frontmatter_blocks(segments: &[Segment]) -> Vec<FrontmatterBlock> {
    segments
        .iter()
        .enumerate()
        .filter_map(|(slide, segment)| {
            let matter = segment.frontmatter.as_ref()?;
            // Counted by splitting rather than by `lines`, which drops a
            // trailing empty line — the blank an author leaves themselves
            // after the last key, and exactly where they ask for the next one.
            let height = matter.text.split('\n').count() as u32;

            Some(FrontmatterBlock {
                slide,
                lines: LineSpan { first: matter.line, last: matter.line + height - 1 },
            })
        })
        .collect()
}

/// Deck frontmatter that has been opened and not yet closed.
///
/// The parser is right to treat this as body text — an unterminated block
/// should show the author their YAML rather than swallow the file. But an
/// editor is asked to complete inside it constantly, because typing the
/// opening `---` and then a key is how every deck starts. What the file
/// *means* and what the author is *in the middle of* are different questions,
/// which is why this is answered here and not in the parser.
fn unclosed_frontmatter(source: &str, separator: &str, total_lines: u32) -> Option<LineSpan> {
    let mut lines = source.lines();

    if !scanner::is_separator_of(lines.next()?, separator) {
        return None;
    }
    if lines.any(|line| scanner::is_separator_of(line, separator)) {
        return None;
    }

    Some(LineSpan { first: 2, last: total_lines.max(2) })
}

fn ends_inside_a_fence(source: &str) -> bool {
    let mut fences = scanner::FenceTracker::new();
    for line in source.lines() {
        fences.feed(line);
    }
    fences.is_inside()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DECK: &str = "---\ntitle: T\n---\n\n# One\n\n---\nlayout: split\n---\n\n# Two\n";

    #[test]
    fn a_slide_spans_from_its_first_line_to_the_line_before_the_next() {
        let analysis = analyze(DECK);

        assert_eq!(analysis.slides.len(), 2);
        assert_eq!(analysis.slides[0], LineSpan { first: 1, last: 7 }, "up to its separator");
        assert_eq!(analysis.slides[1], LineSpan { first: 8, last: 11 });
    }

    #[test]
    fn deck_frontmatter_belongs_to_the_first_slide() {
        // Otherwise line one is outside every symbol and an editor's
        // breadcrumb goes blank at the top of the file.
        assert_eq!(analyze(DECK).slide_at(1), Some(0));
    }

    #[test]
    fn a_separator_ends_the_slide_above_it() {
        let analysis = analyze("# One\n\n---\n\n# Two\n");

        assert_eq!(analysis.slide_at(3), Some(0), "the separator line");
        assert_eq!(analysis.slide_at(5), Some(1));
    }

    #[test]
    fn frontmatter_blocks_are_located_and_attributed() {
        let analysis = analyze(DECK);

        assert_eq!(analysis.frontmatter.len(), 2);
        assert_eq!(analysis.frontmatter[0].lines, LineSpan { first: 2, last: 2 });
        assert!(analysis.frontmatter[0].is_deck());
        assert_eq!(analysis.frontmatter[1].lines, LineSpan { first: 8, last: 8 });
        assert!(!analysis.frontmatter[1].is_deck());
    }

    #[test]
    fn a_multi_line_frontmatter_block_covers_all_of_its_keys() {
        let analysis = analyze("---\ntitle: T\nauthor: A\ntheme: terminal\n---\n\n# One\n");
        let block = analysis.frontmatter_at(4).unwrap();

        assert_eq!(block.lines, LineSpan { first: 2, last: 4 });
    }

    #[test]
    fn frontmatter_being_typed_is_found_before_it_is_closed() {
        // The first thing anyone types into a new deck, and the parser cannot
        // see it as frontmatter until the closing separator exists.
        let analysis = analyze("---\ntitle: T\n");

        assert!(analysis.frontmatter.is_empty(), "the parser is right to see body text");
        assert!(analysis.frontmatter_at(2).is_some_and(|block| block.is_deck()));
    }

    #[test]
    fn a_closed_block_is_not_also_reported_as_unclosed() {
        assert!(analyze(DECK).unclosed_frontmatter.is_none());
    }

    #[test]
    fn a_document_that_does_not_open_with_a_separator_has_no_open_block() {
        assert!(analyze("# One\n").unclosed_frontmatter.is_none());
    }

    #[test]
    fn parse_diagnostics_reach_the_findings() {
        let analysis = analyze("---\ntitle: [oops\n---\n\n# One\n");

        assert!(analysis.findings.iter().any(|d| d.code == "frontmatter/invalid-yaml"));
    }

    #[test]
    fn lint_findings_reach_the_findings_too() {
        // The headline case: a contrast or legibility failure currently shows
        // up at build time, long after the author stopped looking.
        let analysis = analyze("# One\n\n![](./diagram.png)\n");

        assert!(analysis.findings.iter().any(|d| d.code == "structure/missing-alt"));
    }

    #[test]
    fn the_linter_judges_the_theme_the_deck_actually_names() {
        // Linting every deck against the default would report the wrong
        // answer for a deck that chose a different one.
        let terminal = analyze("---\ntheme: terminal\n---\n\n# One\n");
        assert!(terminal.findings.is_empty(), "a built-in theme passes its own linter");
    }

    #[test]
    fn an_unknown_theme_is_reported_once_and_still_lints_against_the_default() {
        // The completion list is where a typo is *prevented*; the dialect check
        // is where a typo already in the file is reported. This crate invents
        // neither — it publishes the finding that crate produced, so the editor
        // and `slidx lint` say the same sentence.
        let analysis = analyze("---\ntheme: termnal\n---\n\n# One\n");
        let codes: Vec<&str> =
            analysis.findings.iter().map(|finding| finding.code.as_str()).collect();

        assert_eq!(codes, vec!["dialect/unknown-theme"], "and no second opinion beside it");
    }

    #[test]
    fn a_step_addressing_a_mark_that_is_not_there_is_reported_as_the_author_types() {
        // The finding worth having in an editor: the stop exists, so nothing
        // downstream complains, and the presenter discovers it on stage.
        let analysis =
            analyze("---\nsteps:\n  - reveal: \"#reuslt\"\n---\n\nThe [result]{#result}.\n");

        assert!(
            analysis.findings.iter().any(|finding| finding.code == "dialect/unknown-target"),
            "{:?}",
            analysis.findings
        );
    }

    #[test]
    fn a_half_typed_fence_is_reported_rather_than_silently_eating_the_deck() {
        let analysis = analyze("# One\n\n```rust\n\n---\n\n# Two\n");

        assert_eq!(analysis.deck.slides.len(), 1, "the separator below is code now");
        assert!(!analysis.outline_is_trustworthy());
    }

    #[test]
    fn a_closed_fence_leaves_the_outline_trustworthy() {
        let analysis = analyze("# One\n\n```rust\nlet a = 1;\n```\n\n---\n\n# Two\n");

        assert_eq!(analysis.deck.slides.len(), 2);
        assert!(analysis.outline_is_trustworthy());
    }

    #[test]
    fn an_empty_document_still_analyses() {
        let analysis = analyze("");

        assert_eq!(analysis.slides.len(), 1);
        assert_eq!(analysis.slides[0], LineSpan { first: 1, last: 1 });
        assert!(analysis.outline_is_trustworthy());
    }

    #[test]
    fn a_deck_written_in_japanese_analyses_the_same_way() {
        let analysis = analyze("---\ntitle: 高速なデッキ\n---\n\n# 導入\n\n---\n\n# まとめ\n");

        assert_eq!(analysis.deck.slides.len(), 2);
        assert_eq!(analysis.deck.slides[1].title.as_deref(), Some("まとめ"));
        assert_eq!(analysis.slide_at(9), Some(1));
    }
}
