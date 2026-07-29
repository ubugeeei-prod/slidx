//! Which version a directory asks for.
//!
//! A project pins its slidx in a `.slidx-version` file holding one line. The
//! search walks up from the working directory, so running a command anywhere
//! inside a repository finds the pin at its root — which is the only behaviour
//! that makes a pin worth having. A file you have to be standing exactly on top
//! of is a file people forget is there.
//!
//! Failing that, `~/.slidx/version` says what to use when nothing asks for
//! anything in particular.
//!
//! ## One format, not two
//!
//! Both files are the same thing: a version, optionally with comments and blank
//! lines around it. That is deliberate — a project pin and a global default
//! answer the same question at different scopes, and giving them two syntaxes
//! would mean learning the second one to do the same thing.
//!
//! ```text
//! # the version this talk was built and rehearsed with
//! 0.2.0
//! ```
//!
//! ## The walk goes all the way up
//!
//! Not to the git root, not to the home directory — to the filesystem root.
//! Stopping at a repository boundary would mean a pin outside one silently did
//! nothing, and "silently did nothing" is the failure mode a pin exists to
//! rule out. [`Pin::file`] reports where the answer came from, so a surprising
//! one is one question to answer rather than a hunt.

use std::fs;
use std::path::{Path, PathBuf};

/// The file a project pins its version in.
pub const PIN_FILE: &str = ".slidx-version";

/// What version to run, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pin {
    /// A `.slidx-version` found by walking up from the working directory.
    Project { version: String, file: PathBuf },
    /// `~/.slidx/version` — what `slidx version use` writes.
    Default { version: String },
    /// Nothing asked for anything.
    Unpinned,
}

impl Pin {
    pub fn version(&self) -> Option<&str> {
        match self {
            Self::Project { version, .. } | Self::Default { version } => Some(version),
            Self::Unpinned => None,
        }
    }

    /// The file this came from, for saying so out loud.
    pub fn file(&self) -> Option<&Path> {
        match self {
            Self::Project { file, .. } => Some(file),
            _ => None,
        }
    }

    /// How to describe where this came from, in one phrase.
    pub fn source(&self) -> String {
        match self {
            Self::Project { file, .. } => format!("pinned by {}", file.display()),
            Self::Default { .. } => "the default for this machine".to_string(),
            Self::Unpinned => "nothing pins a version".to_string(),
        }
    }
}

/// Looks for a project pin, then the machine default.
pub fn resolve(from: &Path, default_file: &Path) -> Pin {
    if let Some((version, file)) = walk_up(from) {
        return Pin::Project { version, file };
    }

    match read(default_file) {
        Some(version) => Pin::Default { version },
        None => Pin::Unpinned,
    }
}

/// The nearest `.slidx-version` at or above `from`.
pub fn walk_up(from: &Path) -> Option<(String, PathBuf)> {
    let mut directory = Some(from);

    while let Some(here) = directory {
        let candidate = here.join(PIN_FILE);

        // A file that exists but says nothing is not an answer — keep walking.
        // Somebody who has commented a pin out means "use whatever is above",
        // not "use nothing".
        if let Some(version) = read(&candidate) {
            return Some((version, candidate));
        }

        directory = here.parent();
    }

    None
}

/// One version from a pin file, or nothing.
///
/// Comments and blank lines are skipped so a pin can say *why* — the version a
/// talk was rehearsed with is exactly the kind of thing that wants a sentence
/// next to it.
pub fn read(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;

    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.trim_start_matches('v').to_string())
        .filter(|version| !version.is_empty())
}

/// Writes a pin file, creating the directory if it is missing.
pub fn write(path: &Path, version: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, format!("{version}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-pin-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");
            Self(path)
        }

        fn dir(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir_all(&path).expect("a directory");
            path
        }

        fn pin(&self, at: &Path, body: &str) -> PathBuf {
            let path = at.join(PIN_FILE);
            fs::write(&path, body).expect("write");
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_pin_at_the_root_of_a_project_is_found_from_anywhere_inside_it() {
        // The only behaviour that makes a pin worth having. One you have to be
        // standing exactly on top of is one people forget is there.
        let scratch = Scratch::new("walk");
        scratch.pin(&scratch.0, "0.2.0\n");
        let deep = scratch.dir("slides/nested/deeper");

        let (version, _) = walk_up(&deep).expect("a pin");
        assert_eq!(version, "0.2.0");
    }

    #[test]
    fn the_nearest_pin_wins_over_one_further_up() {
        // A deck inside a monorepo that needs its own version has to be able
        // to say so without moving out of the monorepo.
        let scratch = Scratch::new("nearest");
        scratch.pin(&scratch.0, "0.1.0\n");
        let inner = scratch.dir("talks/vueconf");
        scratch.pin(&inner, "0.2.0\n");

        assert_eq!(walk_up(&inner).expect("a pin").0, "0.2.0");
    }

    #[test]
    fn a_pin_can_say_why_it_is_there() {
        // The version a talk was rehearsed with is exactly the thing that wants
        // a sentence next to it.
        let scratch = Scratch::new("comment");
        scratch
            .pin(&scratch.0, "# rehearsed against this one, do not bump before the talk\n0.2.0\n");

        assert_eq!(walk_up(&scratch.0).expect("a pin").0, "0.2.0");
    }

    #[test]
    fn a_pin_that_is_only_comments_is_not_an_answer_and_the_walk_continues() {
        // Somebody who commented their pin out means "use whatever is above",
        // not "use nothing".
        let scratch = Scratch::new("commented-out");
        scratch.pin(&scratch.0, "0.1.0\n");
        let inner = scratch.dir("inner");
        scratch.pin(&inner, "# 0.2.0\n");

        assert_eq!(walk_up(&inner).expect("a pin").0, "0.1.0");
    }

    #[test]
    fn a_leading_v_is_accepted_because_that_is_how_tags_are_written() {
        let scratch = Scratch::new("vprefix");
        scratch.pin(&scratch.0, "v0.2.0\n");

        assert_eq!(walk_up(&scratch.0).expect("a pin").0, "0.2.0");
    }

    #[test]
    fn surrounding_whitespace_is_not_part_of_the_version() {
        // An editor that adds a trailing newline, or somebody who indented it.
        let scratch = Scratch::new("space");
        scratch.pin(&scratch.0, "\n\n   0.2.0   \n\n");

        assert_eq!(walk_up(&scratch.0).expect("a pin").0, "0.2.0");
    }

    #[test]
    fn a_pin_reports_the_file_it_came_from() {
        // A surprising version becomes one question to answer rather than a
        // hunt through every directory above you.
        let scratch = Scratch::new("source");
        let file = scratch.pin(&scratch.0, "0.2.0\n");
        let pin = resolve(&scratch.0, Path::new("/nowhere/version"));

        assert_eq!(pin.file(), Some(file.as_path()));
        assert!(pin.source().contains(PIN_FILE), "{}", pin.source());
    }

    #[test]
    fn the_machine_default_answers_when_no_project_pins_anything() {
        let scratch = Scratch::new("default");
        let default = scratch.0.join("version");
        write(&default, "0.3.0").expect("write");

        let pin = resolve(&scratch.dir("elsewhere"), &default);

        assert_eq!(pin.version(), Some("0.3.0"));
        assert_eq!(pin, Pin::Default { version: "0.3.0".into() });
    }

    #[test]
    fn a_project_pin_outranks_the_machine_default() {
        let scratch = Scratch::new("outranks");
        let default = scratch.0.join("version");
        write(&default, "0.3.0").expect("write");
        let project = scratch.dir("project");
        scratch.pin(&project, "0.2.0\n");

        assert_eq!(resolve(&project, &default).version(), Some("0.2.0"));
    }

    #[test]
    fn nothing_anywhere_is_unpinned_rather_than_an_error() {
        // The state of every machine before anyone has chosen. It has to mean
        // "run whatever is installed", not "stop".
        let scratch = Scratch::new("none");

        assert_eq!(resolve(&scratch.0, Path::new("/nowhere/version")), Pin::Unpinned);
        assert_eq!(resolve(&scratch.0, Path::new("/nowhere/version")).version(), None);
    }

    #[test]
    fn a_pin_and_a_default_are_written_and_read_in_the_same_format() {
        // They answer the same question at different scopes. Two syntaxes would
        // mean learning the second one to do the same thing.
        let scratch = Scratch::new("format");
        let default = scratch.0.join("version");
        let project = scratch.0.join(PIN_FILE);

        write(&default, "0.4.0").expect("write");
        write(&project, "0.4.0").expect("write");

        assert_eq!(read(&default), read(&project));
        assert_eq!(fs::read_to_string(&default).unwrap(), fs::read_to_string(&project).unwrap());
    }

    #[test]
    fn writing_a_pin_creates_the_directory_on_a_fresh_machine() {
        let scratch = Scratch::new("mkdir");
        let path = scratch.0.join("nested/deeper/version");

        write(&path, "0.1.0").expect("write");
        assert_eq!(read(&path).as_deref(), Some("0.1.0"));
    }

    #[test]
    fn an_unreadable_pin_file_is_not_a_pin() {
        assert_eq!(read(Path::new("/nowhere/at/all/.slidx-version")), None);
    }
}
