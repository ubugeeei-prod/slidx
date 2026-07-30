//! `slidx create` — a new project, on a deck that already builds.
//!
//! ## What "already builds" rules out
//!
//! A scaffold that needs reading before it works is a scaffold that costs more
//! than it saved. So the four files here are the whole project: a deck that
//! parses and lints clean, a vite config that is one line of configuration, a
//! package.json naming the plugin, and a `.gitignore` for the two directories
//! that are output. There is nothing to fill in and nothing commented out.
//!
//! A test parses the deck this writes and runs the linter over it, so a template
//! that stopped being clean fails here rather than on somebody's first run.
//!
//! ## Why the frontmatter is written by the edit crate
//!
//! Because the values are the author's, and YAML has opinions about them. A talk
//! called `Fast: a story` needs quoting; a duration of `20m` must not gain any.
//! [`slidx_edit`] already decides that, correctly, for the visual editor — so
//! every value passed on the command line goes in through
//! [`EditOp::SetField`](slidx_edit::EditOp::SetField) rather than through a
//! format string that would get it right for most titles.
//!
//! The template itself is a literal, and that is not the same thing: there is no
//! author's file to preserve and no second writer to disagree with, because
//! nothing exists yet. The rule this respects is about editing decks, and it
//! starts applying the moment this command has finished.
//!
//! ## It installs nothing
//!
//! No package manager is run. Which one an author uses is theirs, the network is
//! the one part of this that can fail, and a command that took thirty seconds to
//! create four files would be a command people worked around.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;
use slidx_core::{parse_deck, DeckParseOptions};
use slidx_edit::{apply, EditOp};

use crate::args::Matches;
use crate::home::Home;
use crate::index::{self, Entry};
use crate::lint::source::DEFAULT_DIR;
use crate::style::{Ink, Style};
use crate::Outcome;

/// The deck before anything the author said is written into it.
///
/// One slide with a heading, because a deck of zero slides is not a deck and a
/// heading is what every operation that follows addresses.
const TEMPLATE: &str = "# Untitled deck\n";

/// What a new project is made of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub root: PathBuf,
    /// Relative path and contents, in the order they are written.
    pub files: Vec<(PathBuf, String)>,
}

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let Some(given) = matches.first_positional() else {
        return Outcome::misuse(needs_a_path());
    };

    let root = PathBuf::from(given);
    if let Err(message) = free(&root) {
        return Outcome::misuse(message);
    }

    let project = match plan(&root, matches) {
        Ok(project) => project,
        Err(message) => return Outcome::misuse(message),
    };

    if let Err(message) = write(&project) {
        return Outcome::misuse(message);
    }

    // In the index on the way out, so a project is in `slidx list` before
    // anything has been run on it. Best-effort, as everywhere.
    if let Some(absolute) = root.canonicalize().ok().or_else(|| Some(root.clone())) {
        let deck = parse_deck(&project.deck(), &DeckParseOptions::default());
        index::remember(&Home::discover().index(), Entry::new(absolute).describing(&deck));
    }

    Outcome::out(report(&project, style))
}

/// Everything the project will contain, without touching the disk.
///
/// Separate from writing it so a test can assert on a deck that was never
/// created, and so nothing is half-written when a value is refused.
pub fn plan(root: &Path, matches: &Matches) -> Result<Project, String> {
    let name = directory_name(root);
    let title = matches.value("title").unwrap_or(&name).trim().to_string();

    let mut deck = write_field(TEMPLATE.to_string(), "title", &title)?;
    deck = apply(&deck, &options(), &EditOp::SetHeading { slide: 0.into(), text: title.clone() })
        .map_err(|error| format!("slidx could not write the deck's heading: {error}\n"))?;

    for (key, value) in [
        ("event", matches.value("event")),
        ("duration", matches.value("duration")),
        ("theme", matches.value("theme")),
    ] {
        if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
            deck = write_field(deck, key, value)?;
        }
    }

    Ok(Project {
        root: root.to_path_buf(),
        files: vec![
            (PathBuf::from(DEFAULT_DIR).join("0001.md"), deck),
            (PathBuf::from("vite.config.ts"), VITE_CONFIG.to_string()),
            (PathBuf::from("package.json"), package_json(&name)),
            (PathBuf::from(".gitignore"), GITIGNORE.to_string()),
        ],
    })
}

impl Project {
    /// The deck source, for anything that wants to read what was written.
    pub fn deck(&self) -> String {
        self.files
            .iter()
            .find(|(path, _)| path.extension().is_some_and(|extension| extension == "md"))
            .map(|(_, source)| source.clone())
            .unwrap_or_default()
    }
}

/// One frontmatter key, written the way the editor writes one.
fn write_field(source: String, key: &str, value: &str) -> Result<String, String> {
    apply(
        &source,
        &options(),
        &EditOp::SetField { slide: 0.into(), key: key.to_string(), value: json!(value) },
    )
    .map_err(|error| format!("slidx could not write `{key}` into the deck: {error}\n"))
}

fn options() -> DeckParseOptions {
    DeckParseOptions::default()
}

/// Refuses a destination that already holds something.
///
/// An empty directory is fine — `mkdir talk && cd talk && slidx create .` is a
/// reasonable way to arrive here. Anything with a file in it is somebody's work,
/// and a scaffold is not worth overwriting it for.
fn free(root: &Path) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(root) else {
        return Ok(());
    };

    if entries.flatten().next().is_some() {
        return Err(format!(
            "{} already has something in it.\n\n\
             slidx will not write a new deck over work that is already there. Give it a\n\
             directory that does not exist, or an empty one.\n",
            root.display()
        ));
    }

    Ok(())
}

fn write(project: &Project) -> Result<(), String> {
    for (relative, contents) in &project.files {
        let path = project.root.join(relative);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Could not make {}: {error}\n", parent.display()))?;
        }

        fs::write(&path, contents)
            .map_err(|error| format!("Could not write {}: {error}\n", path.display()))?;
    }

    Ok(())
}

/// The directory's own name, as the default title and the package name.
fn directory_name(root: &Path) -> String {
    root.canonicalize()
        .ok()
        .as_deref()
        .and_then(Path::file_name)
        .or_else(|| root.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "deck".to_string())
}

/// The whole configuration, which is the claim the plugin makes.
const VITE_CONFIG: &str = "\
import { defineConfig } from \"vite\";

import { slidx } from \"@slidx/vite-plugin\";

// The whole configuration. `slidx()` finds ./slides, serves them and the editor
// in dev, and on build emits one HTML page per slide, a PDF and social cards.
export default defineConfig({
  plugins: [slidx()],
});
";

/// `dist` is the build and `node_modules` is somebody else's code. Both are
/// derived from what is committed, and neither belongs in a talk's history.
const GITIGNORE: &str = "dist/\nnode_modules/\n";

/// The project's manifest.
///
/// The plugin is pinned to the version of slidx that wrote it, because the two
/// are released together and a deck built by one and linted by the other is the
/// disagreement this project exists to avoid.
fn package_json(name: &str) -> String {
    let slug = slugged(name);

    format!(
        "{{\n  \"name\": \"{slug}\",\n  \"private\": true,\n  \"type\": \"module\",\n  \
         \"scripts\": {{\n    \"dev\": \"vite\",\n    \"build\": \"vite build\",\n    \
         \"preview\": \"vite preview\"\n  }},\n  \"devDependencies\": {{\n    \
         \"@slidx/vite-plugin\": \"^{version}\",\n    \"vite\": \"^7.3.6\"\n  }}\n}}\n",
        version = crate::version()
    )
}

/// A package name npm will accept: lowercase, and nothing exotic.
///
/// A directory called `Vue Fes 2026` is an ordinary place to keep a talk and an
/// invalid package name, and a manifest npm refuses to read is a project that
/// does not install.
fn slugged(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|character| match character {
            'A'..='Z' => character.to_ascii_lowercase(),
            'a'..='z' | '0'..='9' | '-' | '_' => character,
            _ => '-',
        })
        .collect();

    let slug = slug.trim_matches(['-', '_', '.']).to_string();

    if slug.is_empty() {
        "deck".to_string()
    } else {
        slug
    }
}

fn report(project: &Project, style: &Style) -> String {
    let mut text = format!("  {}\n", style.paint(Ink::Pass, project.root.display()));

    for (relative, _) in &project.files {
        text.push_str(&format!("  {}\n", style.paint(Ink::Faint, relative.display())));
    }

    text.push_str(&format!(
        "\n  {}\n\n    cd {}\n    npm install\n    npm run dev\n",
        style.paint(Ink::Strong, "Next:"),
        quoted(&project.root)
    ));

    text
}

/// A path as a shell would need it written.
///
/// Only for the "next" lines, which somebody copies. A deck kept in `Vue Fes
/// 2026/` is ordinary, and an instruction that breaks when followed is worse
/// than no instruction.
fn quoted(path: &Path) -> String {
    let text = path.display().to_string();

    if text.contains(char::is_whitespace) {
        return format!("\"{text}\"");
    }

    text
}

fn needs_a_path() -> String {
    "`slidx create` needs somewhere to put the deck.\n\n\
     \x20 slidx create ~/talks/vueconf --title \"Making decks fast\" --duration 20m\n\n\
     The directory is made for you, and an existing one is only used if it is empty.\n"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::Severity;
    use slidx_lint::{lint, LintInput, LintOptions};

    fn matches_for(line: &str) -> Matches {
        let argv: Vec<String> = shell_words(line);

        match crate::args::parse(&argv) {
            crate::args::Invocation::Run(_, matches) => matches,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    /// Splits a test's command line, keeping double-quoted values whole.
    fn shell_words(line: &str) -> Vec<String> {
        let mut words = Vec::new();
        let mut current = String::new();
        let mut quoted = false;

        for character in line.chars() {
            match character {
                '"' => quoted = !quoted,
                ' ' if !quoted => {
                    if !current.is_empty() {
                        words.push(std::mem::take(&mut current));
                    }
                }
                _ => current.push(character),
            }
        }

        if !current.is_empty() {
            words.push(current);
        }

        words
    }

    fn planned(line: &str) -> Project {
        plan(Path::new("/talks/vueconf"), &matches_for(line)).expect("a project")
    }

    #[test]
    fn the_deck_a_new_project_starts_with_parses_and_lints_clean() {
        // The whole promise of the command. A scaffold that has to be fixed
        // before it works is one that has to be read, and the first thing
        // somebody does with a new deck should be writing the talk.
        let project = planned("create ~/talks/vueconf --title \"Making decks fast\" --duration 20m");
        let deck = parse_deck(&project.deck(), &options());

        assert!(deck.diagnostics.is_empty(), "{:?}", deck.diagnostics);
        assert_eq!(deck.slides.len(), 1);

        let theme = slidx_theme::default_theme();
        let findings = lint(&LintInput::new(&deck, &theme.surfaces()), &LintOptions::default());
        let blocking: Vec<_> =
            findings.iter().filter(|finding| finding.severity == Severity::Error).collect();

        assert!(blocking.is_empty(), "{blocking:?}");
    }

    #[test]
    fn what_the_author_said_is_in_the_frontmatter_and_on_the_title_slide() {
        let project = planned("create ~/talks/vueconf --title \"Making decks fast\" --event \"Vue Fes\" --duration 20m --theme editorial");
        let deck = parse_deck(&project.deck(), &options());

        assert_eq!(deck.meta.title.as_deref(), Some("Making decks fast"));
        assert_eq!(deck.meta.talk.event.as_deref(), Some("Vue Fes"));
        assert_eq!(deck.meta.duration_seconds, Some(1200));
        assert_eq!(deck.meta.theme.as_deref(), Some("editorial"));
        assert_eq!(deck.slides[0].display_title(), "Making decks fast");
    }

    #[test]
    fn a_title_that_yaml_would_read_as_something_else_is_quoted_by_the_edit_crate() {
        // `Fast: a story` in a frontmatter block is a nested map, and a title of
        // `true` is a boolean. Neither is a case a format string gets right, and
        // neither is this command's to decide.
        let project = planned("create ~/talks/x --title \"Fast: a story\"");
        let deck = parse_deck(&project.deck(), &options());

        assert_eq!(deck.meta.title.as_deref(), Some("Fast: a story"));
        assert!(project.deck().contains('"'), "{}", project.deck());
    }

    #[test]
    fn a_deck_with_nothing_declared_is_titled_after_its_directory() {
        // Better than "Untitled deck", which is what the template says and what
        // nobody means.
        let project = planned("create ~/talks/x");
        let deck = parse_deck(&project.deck(), &options());

        assert_eq!(deck.meta.title.as_deref(), Some("vueconf"));
        assert_eq!(deck.slides[0].display_title(), "vueconf");
    }

    #[test]
    fn a_project_is_four_files_and_the_deck_is_where_the_plugin_looks_for_it() {
        let project = planned("create ~/talks/x");
        let names: Vec<String> =
            project.files.iter().map(|(path, _)| path.display().to_string()).collect();

        assert!(names.contains(&format!("{DEFAULT_DIR}/0001.md")), "{names:?}");
        assert!(names.contains(&"vite.config.ts".to_string()), "{names:?}");
        assert!(names.contains(&"package.json".to_string()), "{names:?}");
        assert!(names.contains(&".gitignore".to_string()), "{names:?}");
    }

    #[test]
    fn the_vite_config_is_the_one_line_of_configuration_the_plugin_claims() {
        // The README says `plugins: [slidx()]` is the whole configuration. A
        // scaffold that wrote more would be the first thing to contradict it.
        assert!(VITE_CONFIG.contains("plugins: [slidx()]"), "{VITE_CONFIG}");
        assert!(!VITE_CONFIG.contains("srcDir"), "{VITE_CONFIG}");
    }

    #[test]
    fn the_manifest_pins_the_plugin_to_the_slidx_that_wrote_it() {
        // The two are released together, and a deck built by one version and
        // linted by another is the disagreement this project exists to avoid.
        let manifest = package_json("vueconf");

        assert!(manifest.contains(&format!("\"@slidx/vite-plugin\": \"^{}\"", crate::version())), "{manifest}");
        assert!(manifest.contains("\"private\": true"), "{manifest}");
    }

    #[test]
    fn the_manifest_is_json_a_package_manager_will_read() {
        let manifest = package_json("Vue Fes 2026");
        let parsed: serde_json::Value = serde_json::from_str(&manifest).expect("valid json");

        assert_eq!(parsed["name"], json!("vue-fes-2026"));
        assert!(parsed["devDependencies"]["vite"].is_string());
    }

    #[test]
    fn a_directory_name_that_is_not_a_valid_package_name_is_made_into_one() {
        // `Vue Fes 2026` is an ordinary place to keep a talk and an invalid
        // package name. A manifest npm refuses to read is a project that does
        // not install.
        assert_eq!(slugged("Vue Fes 2026"), "vue-fes-2026");
        assert_eq!(slugged("発表"), "deck");
        assert_eq!(slugged("--"), "deck");
    }

    #[test]
    fn build_output_and_dependencies_are_ignored_by_git_from_the_start() {
        assert!(GITIGNORE.contains("dist/"));
        assert!(GITIGNORE.contains("node_modules/"));
    }

    #[test]
    fn a_directory_with_something_in_it_is_refused_rather_than_written_over() {
        let scratch =
            std::env::temp_dir().join(format!("slidx-create-taken-{}", std::process::id()));
        let _ = fs::remove_dir_all(&scratch);
        fs::create_dir_all(&scratch).expect("scratch");
        fs::write(scratch.join("notes.md"), "somebody's work").expect("write");

        let message = free(&scratch).expect_err("refused");
        assert!(message.contains("already has something in it"), "{message}");

        let _ = fs::remove_dir_all(&scratch);
    }

    #[test]
    fn an_empty_directory_is_a_fine_place_to_start() {
        // `mkdir talk && cd talk && slidx create .` is a reasonable way to
        // arrive here.
        let scratch =
            std::env::temp_dir().join(format!("slidx-create-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&scratch);
        fs::create_dir_all(&scratch).expect("scratch");

        assert!(free(&scratch).is_ok());
        assert!(free(&scratch.join("not-there-yet")).is_ok());

        let _ = fs::remove_dir_all(&scratch);
    }

    #[test]
    fn the_project_is_written_where_it_was_asked_for() {
        let scratch =
            std::env::temp_dir().join(format!("slidx-create-write-{}", std::process::id()));
        let _ = fs::remove_dir_all(&scratch);

        let project = plan(&scratch, &matches_for("create x --title Talk")).expect("a project");
        write(&project).expect("written");

        assert!(scratch.join("slides/0001.md").is_file());
        assert!(scratch.join("vite.config.ts").is_file());
        assert_eq!(
            fs::read_to_string(scratch.join("slides/0001.md")).expect("read"),
            project.deck()
        );

        let _ = fs::remove_dir_all(&scratch);
    }

    #[test]
    fn the_next_steps_are_quoted_where_a_shell_would_need_them_to_be() {
        // An instruction that breaks when followed is worse than none, and a
        // talk in `Vue Fes 2026/` is ordinary.
        let project = Project { root: PathBuf::from("/talks/Vue Fes 2026"), files: Vec::new() };
        let text = report(&project, &Style::plain());

        assert!(text.contains("cd \"/talks/Vue Fes 2026\""), "{text}");
    }

    #[test]
    fn the_report_says_what_to_run_next_because_nothing_was_installed() {
        let text = report(&planned("create ~/talks/x"), &Style::plain());

        assert!(text.contains("install"), "{text}");
        assert!(text.contains("dev"), "{text}");
    }

    #[test]
    fn no_path_at_all_shows_what_a_whole_invocation_looks_like() {
        let message = needs_a_path();

        assert!(message.contains("slidx create"), "{message}");
        assert!(message.contains("--duration"), "{message}");
    }
}
