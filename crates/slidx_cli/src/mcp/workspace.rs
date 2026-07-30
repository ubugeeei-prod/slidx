//! Which decks this server will answer about.
//!
//! An MCP server is handed paths by a model, and a model gets its paths from
//! whatever it has been reading — which, when the subject is a deck, includes
//! the deck. So the set of files this server will open is decided once, here,
//! from the command line, and no argument can widen it.
//!
//! Two sets, because reading and writing are not the same risk:
//!
//! **The roots** are the directories `slidx mcp` was started in or pointed at.
//! They are the only places it will ever write, and only when [`Authority`] says
//! so.
//!
//! **The index** is every project this machine has run a slidx command on,
//! which [`crate::index`] fills by itself. Reading those is the feature that
//! makes reusing a slide from a talk given eighteen months ago possible at all,
//! and they are the speaker's own decks by construction — nothing gets into that
//! file except by them running slidx on it. They are readable and never
//! writable: a server pointed at one project must not be able to rewrite a talk
//! somebody gave last year because a slide in front of it mentioned the path.

use std::path::{Path, PathBuf};

use slidx_core::{parse_deck, Deck, DeckParseOptions};

use crate::home::Home;
use crate::index::Index;
use crate::lint::source::{self, DEFAULT_DIR};

/// The default slide separator, matching the plugin and `slidx lint`.
pub const SEPARATOR: &str = "---";

/// What this server is allowed to do to a deck.
///
/// Read-only unless the person who started it said otherwise, because an MCP
/// server that will rewrite a conference talk because something in the deck told
/// it to is a liability. Nothing a resource contains can change this: the value
/// comes from the command line and is never recomputed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Authority {
    #[default]
    ReadOnly,
    /// Operations may be applied to decks under a root.
    Write,
}

impl Authority {
    pub fn writes(self) -> bool {
        self == Self::Write
    }
}

/// A deck the server has read, and where it came from.
#[derive(Debug, Clone)]
pub struct Reading {
    /// The deck path as resolved: a file, or the directory of slide files.
    pub path: PathBuf,
    /// What to call this deck in a message.
    pub label: String,
    /// The joined source, exactly as the parser saw it.
    pub source: String,
    pub deck: Deck,
    /// The files it came from, in deck order. Empty for a single-file deck.
    ///
    /// Held because a splice is measured in the joined source and has to be cut
    /// back into the files it came from — see [`super::deck`].
    pub files: Vec<PathBuf>,
    /// The separator the deck was joined with, so a later edit measures the same
    /// document this reading did.
    pub separator: String,
}

/// The directories this server will open a file in.
#[derive(Debug, Clone)]
pub struct Workspace {
    roots: Vec<PathBuf>,
    index: PathBuf,
    authority: Authority,
}

impl Workspace {
    /// The directories the command line named.
    ///
    /// Resolved to their real paths here rather than at each comparison: on
    /// macOS the temporary directory is reached through a symlink, and a
    /// containment check between a resolved path and an unresolved root would
    /// refuse a directory the server was explicitly given.
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            roots: roots.iter().map(|root| resolved(root)).collect(),
            index: Home::discover().index(),
            authority: Authority::default(),
        }
    }

    /// Reads the deck index from somewhere else. For a test, or an embedder.
    pub fn with_index(mut self, path: PathBuf) -> Self {
        self.index = path;
        self
    }

    /// Lets operations be applied. `slidx mcp --write`, and nothing else.
    pub fn writing(mut self) -> Self {
        self.authority = Authority::Write;
        self
    }

    pub fn authority(&self) -> Authority {
        self.authority
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Every directory a file may be opened under, roots first.
    ///
    /// Recomputed per call rather than cached, because the index fills itself
    /// while a session is open: a `slidx lint` run in another terminal should
    /// not need this server restarted to be visible.
    pub fn projects(&self) -> Vec<PathBuf> {
        let mut projects = self.roots.clone();

        for entry in Index::load(&self.index).live() {
            let project = resolved(&entry.path);
            if !projects.contains(&project) {
                projects.push(project);
            }
        }

        projects
    }

    /// A path the server is allowed to open, resolved.
    pub fn readable(&self, path: &str) -> Result<PathBuf, String> {
        let Ok(full) = PathBuf::from(path).canonicalize() else {
            return Err(format!(
                "There is nothing at {path}. A deck is a Markdown file, or a directory of \
                 slide files named so they sort — {DEFAULT_DIR}/0001.md and so on."
            ));
        };

        if self.projects().iter().any(|project| full.starts_with(project)) {
            return Ok(full);
        }

        Err(format!(
            "{path} is outside the directories this server may open.\n\nIt serves {}, and \
             every project this machine has run a slidx command on. Start `slidx mcp` in the \
             project you want it to read, or pass --root <path>.",
            self.roots.iter().map(|root| root.display().to_string()).collect::<Vec<_>>().join(", ")
        ))
    }

    /// Reads and parses a deck.
    ///
    /// A directory holding no slide files but with a `slides/` in it is read as
    /// the project rather than refused, because that is the layout
    /// `@slidx/vite-plugin` builds and the one `slidx lint` defaults to. A model
    /// that passes the project directory means the deck in it.
    pub fn read_deck(&self, path: &str, separator: Option<&str>) -> Result<Reading, String> {
        self.deck_at(self.readable(path)?, separator)
    }

    /// Reads a deck this server is allowed to change.
    ///
    /// Two refusals rather than one, because they are different problems and a
    /// model can only act on the second: a server started read-only has to be
    /// restarted, and a deck outside every root is the wrong path.
    pub fn edit_deck(&self, path: &str, separator: Option<&str>) -> Result<Reading, String> {
        if !self.authority.writes() {
            return Err(format!(
                "This server is read-only, so nothing was changed. `{path}` was not written. \
                 Whoever started it can allow writes with `slidx mcp --write`; that is their \
                 decision to make and not one a deck or a tool call can make for them."
            ));
        }

        let resolved = self.readable(path)?;

        if !self.roots.iter().any(|root| resolved.starts_with(root)) {
            return Err(format!(
                "{path} can be read but not written: it is a project the deck index knows \
                 about rather than one this server was pointed at. Start `slidx mcp` there, or \
                 pass --root, if it is meant to be edited."
            ));
        }

        self.deck_at(resolved, separator)
    }

    fn deck_at(&self, resolved: PathBuf, separator: Option<&str>) -> Result<Reading, String> {
        let separator = separator.unwrap_or(SEPARATOR);

        let slides = resolved.join(DEFAULT_DIR);
        let target = if slides.is_dir() { slides } else { resolved };

        let read = source::read(&target, separator)?;
        let options = DeckParseOptions { separator: separator.to_string(), ..Default::default() };

        // Joined the way the build joins, not the way `source::read` does. The
        // two differ, and the one that matters is the deck a browser gets.
        let files = super::deck::read_files(&target, &read.files)?;
        let source = super::deck::join(&files, separator).source;

        Ok(Reading {
            path: target,
            label: read.label,
            deck: parse_deck(&source, &options),
            source,
            files: read.files,
            separator: separator.to_string(),
        })
    }
}

/// A path's real location, or the path itself when it does not exist.
///
/// An unresolvable root is kept rather than dropped so that the refusal names
/// what the server was actually given. A root that is a typo should read as a
/// directory nothing is under, not vanish from the message.
fn resolved(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("slidx-ws-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(path.join("slides")).expect("scratch");
            Self(path)
        }

        fn slide(&self, name: &str, body: &str) {
            fs::write(self.0.join("slides").join(name), body).expect("write");
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A workspace over one project, and an index nothing has written.
    fn workspace(scratch: &Scratch) -> Workspace {
        Workspace::new(vec![scratch.path().to_path_buf()])
            .with_index(scratch.path().join("no-index.json"))
    }

    #[test]
    fn a_deck_under_a_root_is_read() {
        let scratch = Scratch::new("read");
        scratch.slide("0001.md", "# One\n");

        let reading = workspace(&scratch)
            .read_deck(&scratch.path().join("slides").display().to_string(), None)
            .expect("a deck");

        assert_eq!(reading.deck.slides.len(), 1);
        assert_eq!(reading.source, "# One");
    }

    #[test]
    fn the_project_directory_means_the_deck_inside_it() {
        // What a model passes when it has been given a repository to work in.
        let scratch = Scratch::new("project");
        scratch.slide("0001.md", "# One\n\n---\n\n# Two\n");

        let reading = workspace(&scratch)
            .read_deck(&scratch.path().display().to_string(), None)
            .expect("a deck");

        assert_eq!(reading.deck.slides.len(), 2);
        assert!(reading.path.ends_with(DEFAULT_DIR));
    }

    #[test]
    fn a_path_outside_every_root_is_refused_and_the_message_names_the_roots() {
        // A model gets its paths from what it has been reading, and when the
        // subject is a deck that includes the deck. An argument cannot widen
        // what the command line decided.
        let scratch = Scratch::new("outside");
        // The directory the scratch sits in: it exists on every platform this
        // ships to, and it is not under the one root this server was given.
        let above = std::env::temp_dir().display().to_string();
        let refusal = workspace(&scratch).read_deck(&above, None).expect_err("outside every root");

        assert!(refusal.contains("outside"), "{refusal}");
        assert!(refusal.contains(&scratch.path().canonicalize().unwrap().display().to_string()));
    }

    #[test]
    fn a_path_that_does_not_exist_says_what_a_deck_looks_like() {
        let scratch = Scratch::new("missing");
        let refusal = workspace(&scratch)
            .read_deck(&scratch.path().join("nowhere").display().to_string(), None)
            .expect_err("nothing there");

        assert!(refusal.contains("0001.md"), "{refusal}");
    }

    #[test]
    fn a_project_the_index_knows_about_is_readable_without_being_a_root() {
        // The feature this exists for: reusing a slide from a talk given
        // eighteen months ago, in a repository nobody remembers the path of.
        let elsewhere = Scratch::new("indexed");
        elsewhere.slide("0001.md", "# Last year\n");

        let index = elsewhere.path().join("index.json");
        crate::index::remember(&index, crate::index::Entry::new(elsewhere.path()));

        let here = Scratch::new("here");
        let workspace = Workspace::new(vec![here.path().to_path_buf()]).with_index(index.clone());

        let reading = workspace
            .read_deck(&elsewhere.path().display().to_string(), None)
            .expect("an indexed project");

        assert_eq!(reading.deck.slides[0].title.as_deref(), Some("Last year"));
    }

    #[test]
    fn an_unresolvable_root_still_appears_in_the_refusal_it_causes() {
        // A typo in --root has to read as a directory nothing is under, not
        // vanish from the message and leave somebody wondering what is served.
        let workspace = Workspace::new(vec![PathBuf::from("/nowhere/at/all")])
            .with_index(PathBuf::from("/nowhere/index.json"));

        let refusal =
            workspace.readable(&std::env::temp_dir().display().to_string()).expect_err("outside");
        assert!(refusal.contains("/nowhere/at/all"), "{refusal}");
    }

    #[test]
    fn a_decks_own_separator_is_honoured_when_the_caller_names_one() {
        let scratch = Scratch::new("separator");
        scratch.slide("0001.md", "# One\n\n***\n\n# Two\n");

        let reading = workspace(&scratch)
            .read_deck(&scratch.path().display().to_string(), Some("***"))
            .expect("a deck");

        assert_eq!(reading.deck.slides.len(), 2);
    }
}
