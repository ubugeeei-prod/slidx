//! Talking to git, through the program the author already has.
//!
//! No git library. A deck lives in the author's repository, with their hooks,
//! their `core.autocrlf`, their signing key, their `.gitattributes` and their
//! submodules — and a reimplementation of git that agreed with all of that on
//! every platform is not a thing anybody should attempt for one command. The
//! binary on PATH is the one whose behaviour their repository is already
//! configured for.
//!
//! ## What this deliberately does not do
//!
//! It never fetches, never pushes, never rewrites history and never touches a
//! branch. `slidx save` makes a commit and `slidx rm` reads a status; both are
//! local and both are things the author could type. Anything that talks to a
//! remote is theirs to run, because that is the operation with consequences
//! somebody else can see.
//!
//! ## Paths, not shell
//!
//! Every argument is passed as an argument. Nothing here builds a command line
//! as a string, so a deck in `~/talks/Vue Fes 2026` needs no quoting and a
//! directory whose name begins with a dash is separated by `--` rather than
//! hoped about.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A git repository, and the directory it is rooted at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    root: PathBuf,
}

/// One path git reports as different from HEAD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The two-letter porcelain code: `M `, ` M`, `??`, `A `, and so on.
    pub status: String,
    /// Relative to the repository root, which is how git prints it.
    pub path: PathBuf,
}

impl Change {
    /// True for a file git has never been told about.
    pub fn is_untracked(&self) -> bool {
        self.status == "??"
    }
}

impl Repo {
    /// The repository a path is inside, if it is inside one.
    ///
    /// Asks git rather than looking for a `.git` directory: a worktree's `.git`
    /// is a file, a submodule's points elsewhere, and `GIT_DIR` overrides both.
    pub fn discover(path: &Path) -> Option<Self> {
        let output = run(path, &["rev-parse", "--show-toplevel"]).ok()?;

        Some(Self { root: PathBuf::from(output.trim()) })
    }

    /// Starts a repository in `path`.
    pub fn init(path: &Path) -> Result<Self, String> {
        run(path, &["init"])?;

        Self::discover(path).ok_or_else(|| "git init made no repository".to_string())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// True once there is a commit to compare against.
    ///
    /// A repository with none is the state `git init` leaves, and every command
    /// that reads HEAD has to have an answer for it rather than an error.
    pub fn has_commits(&self) -> bool {
        run(&self.root, &["rev-parse", "--verify", "HEAD"]).is_ok()
    }

    /// What has changed under one directory, staged or not, tracked or not.
    ///
    /// `-z` rather than the default quoting: git escapes unusual bytes in a
    /// path unless asked not to, and a deck in a directory with a space or a
    /// non-ASCII name is ordinary. This is the read that decides what a commit
    /// will contain, so it cannot be the place a path is mangled.
    pub fn changes(&self, within: &Path) -> Result<Vec<Change>, String> {
        let within = self.relative(within);
        let output = run(
            &self.root,
            &["status", "--porcelain", "-z", "--untracked-files=all", "--", path_arg(&within)],
        )?;

        Ok(parse_status(&output))
    }

    /// A file as HEAD has it, or `None` when HEAD does not have it.
    ///
    /// The other half of a deck-shaped diff: the deck as it was committed. A
    /// file that is new is simply absent, which is what makes it a new slide
    /// rather than a changed one.
    pub fn committed(&self, file: &Path) -> Option<String> {
        let relative = self.relative(file);
        let spec = format!("HEAD:{}", slashed(&relative));

        run(&self.root, &["show", &spec]).ok()
    }

    /// The `.md` files one directory held at HEAD, in deck order.
    ///
    /// Sorted by name, the same rule [`crate::lint::source`] reads a deck with.
    /// A deck whose files are ordered one way on disk and another at HEAD would
    /// diff as a reorder that nobody performed.
    pub fn committed_files(&self, directory: &Path) -> Vec<PathBuf> {
        let within = self.relative(directory);
        let Ok(output) = run(
            &self.root,
            &["ls-tree", "-r", "--name-only", "-z", "HEAD", "--", path_arg(&within)],
        ) else {
            return Vec::new();
        };

        let mut files: Vec<PathBuf> = output
            .split('\0')
            .filter(|line| !line.is_empty())
            .filter(|line| line.to_lowercase().ends_with(".md"))
            .map(|line| self.root.join(line))
            .collect();

        files.sort();
        files
    }

    /// Stages everything under a path, ignored files excepted.
    ///
    /// Needed for one reason: a new slide is an untracked file, and git will
    /// not commit a path it has never been told about. Naming a directory
    /// rather than a file also means `.gitignore` still decides — `git add` on
    /// a directory skips what is ignored, where naming an ignored file
    /// outright is an error.
    pub fn stage(&self, path: &Path) -> Result<(), String> {
        let relative = self.relative(path);

        run(&self.root, &["add", "--", path_arg(&relative)]).map(|_| ())
    }

    /// Commits the working-tree content of these paths, and nothing else.
    ///
    /// Passing paths to `git commit` makes it ignore the index for everything
    /// else, which is the property `slidx save` is built on: an author with
    /// something else staged gets a commit of their deck, not of both. Without
    /// it, one command would quietly sweep up work they were part-way through.
    pub fn commit(&self, message: &str, paths: &[PathBuf]) -> Result<(), String> {
        let mut arguments =
            vec!["commit".to_string(), "--message".to_string(), message.to_string()];
        arguments.push("--".to_string());

        for path in paths {
            arguments.push(slashed(&self.relative(path)));
        }

        let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
        run(&self.root, &borrowed).map(|_| ())
    }

    /// A path as git wants to hear it: relative to the root.
    fn relative(&self, path: &Path) -> PathBuf {
        let full = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let root = self.root.canonicalize().unwrap_or_else(|_| self.root.clone());

        full.strip_prefix(&root).map(Path::to_path_buf).unwrap_or(full)
    }
}

/// True when there is a git to run at all.
///
/// Somebody can have a deck and no git — a downloaded template, a machine
/// somebody else set up. That is a thing to say plainly, not to crash on.
pub fn available() -> bool {
    Command::new("git")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// The root of the current directory, when there is one.
pub fn here() -> Option<Repo> {
    Repo::discover(&std::env::current_dir().ok()?)
}

/// Runs git in a directory and hands back its standard output.
///
/// A non-zero exit becomes the error, with git's own message in it. git's
/// diagnostics are better than anything this module could write — "fatal: not a
/// git repository" says exactly what is wrong — so they are passed through
/// rather than replaced.
fn run(directory: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(directory)
        .args(arguments)
        // A pager would hang waiting for a keypress, and a prompt for
        // credentials has nothing to prompt for here.
        .env("GIT_PAGER", "cat")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|error| format!("could not run git: {error}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// A path as one argument to git, lossily where it is not UTF-8.
///
/// A path that is not UTF-8 exists on both Unix and Windows, and the lossy form
/// is wrong for it — but the alternative is refusing to look at the repository
/// at all. git will report that it cannot find the mangled name, which is a
/// clearer failure than silence.
fn path_arg(path: &Path) -> &str {
    match path.to_str() {
        // The repository root is the empty relative path, and an empty pathspec
        // is a fatal error rather than "everything". `slidx save --all` in a
        // project that is itself the repository is that case.
        Some("") | None => ".",
        Some(text) => text,
    }
}

/// A relative path spelled the way git spells one: forward slashes, on every
/// platform. `git show HEAD:slides\0001.md` finds nothing on Windows.
fn slashed(path: &Path) -> String {
    let joined = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");

    if joined.is_empty() {
        return ".".to_string();
    }

    joined
}

/// Reads `git status --porcelain -z`.
///
/// A record is two status letters, a space, and the path. A rename is followed
/// by a second NUL-separated field holding the old path, which is skipped: what
/// callers here want is what a commit would contain, and that is the new one.
fn parse_status(output: &str) -> Vec<Change> {
    let mut fields = output.split('\0').filter(|field| !field.is_empty());
    let mut changes = Vec::new();

    while let Some(record) = fields.next() {
        if record.len() < 4 {
            continue;
        }

        let (status, path) = record.split_at(2);
        let status = status.to_string();

        // R and C carry the source path in the next field.
        if status.starts_with(['R', 'C']) {
            let _ = fields.next();
        }

        changes.push(Change { status, path: PathBuf::from(path.trim_start()) });
    }

    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A real repository in a scratch directory.
    ///
    /// Real rather than mocked, because what is being tested is agreement with
    /// git — and a mock of git agrees with whatever this module already
    /// believes.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-git-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");

            let scratch = Self(path);
            let _ = run(&scratch.0, &["init"]);
            // A commit needs an identity, and the machine running the tests may
            // have none configured.
            let _ = run(&scratch.0, &["config", "user.email", "tests@slidx.invalid"]);
            let _ = run(&scratch.0, &["config", "user.name", "slidx tests"]);
            let _ = run(&scratch.0, &["config", "commit.gpgsign", "false"]);

            scratch
        }

        fn write(&self, relative: &str, body: &str) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().expect("a parent")).expect("directories");
            fs::write(&path, body).expect("write");
            path
        }

        fn repo(&self) -> Repo {
            Repo::discover(&self.0).expect("a repository")
        }

        fn log(&self) -> String {
            run(&self.0, &["log", "--pretty=%s%n%b", "--name-only"]).unwrap_or_default()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn git_is_here() -> bool {
        available()
    }

    #[test]
    fn a_directory_that_is_not_a_repository_is_not_reported_as_one() {
        let path = std::env::temp_dir().join(format!("slidx-git-none-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch");

        // A temp directory inside a repository would find that one, which is
        // why this asserts on the answer rather than on None.
        let found = Repo::discover(&path);
        assert!(found.is_none() || found.expect("a repo").root() != path);

        let _ = fs::remove_dir_all(&path);
    }

    #[test]
    fn a_fresh_repository_has_no_commits_to_compare_against() {
        if !git_is_here() {
            return;
        }

        // The state `git init` leaves, and the state a new deck is saved from.
        let scratch = Scratch::new("fresh");
        assert!(!scratch.repo().has_commits());
    }

    #[test]
    fn a_new_file_is_reported_as_untracked_and_a_changed_one_as_modified() {
        if !git_is_here() {
            return;
        }

        let scratch = Scratch::new("status");
        scratch.write("slides/0001.md", "# One\n");
        let repo = scratch.repo();
        repo.stage(&scratch.0.join("slides")).expect("stage");
        repo.commit("first", &[scratch.0.join("slides")]).expect("commit");

        scratch.write("slides/0001.md", "# One, changed\n");
        scratch.write("slides/0002.md", "# Two\n");

        let changes = repo.changes(&scratch.0.join("slides")).expect("status");
        let untracked: Vec<&Change> =
            changes.iter().filter(|change| change.is_untracked()).collect();

        assert_eq!(changes.len(), 2, "{changes:?}");
        assert_eq!(untracked.len(), 1, "{changes:?}");
        assert_eq!(untracked[0].path, PathBuf::from("slides/0002.md"));
    }

    #[test]
    fn a_path_with_a_space_in_it_is_reported_unmangled() {
        if !git_is_here() {
            return;
        }

        // git escapes unusual bytes in a path unless told not to, and a deck in
        // `Vue Fes 2026/` is ordinary. This is the read that decides what a
        // commit contains, so it cannot be where a path is mangled.
        let scratch = Scratch::new("spaces");
        scratch.write("Vue Fes 2026/slides/0001.md", "# One\n");

        let changes = scratch.repo().changes(&scratch.0.join("Vue Fes 2026")).expect("status");

        assert_eq!(changes.len(), 1, "{changes:?}");
        assert_eq!(changes[0].path, PathBuf::from("Vue Fes 2026/slides/0001.md"));
    }

    #[test]
    fn only_the_paths_named_are_committed_even_with_something_else_staged() {
        if !git_is_here() {
            return;
        }

        // The property `slidx save` is built on. An author part-way through
        // something else must not find it swept into a commit about slides.
        let scratch = Scratch::new("only");
        scratch.write("slides/0001.md", "# One\n");
        scratch.write("notes.txt", "half-finished\n");
        let repo = scratch.repo();

        repo.stage(&scratch.0.join("notes.txt")).expect("stage");
        repo.stage(&scratch.0.join("slides")).expect("stage");
        repo.commit("the deck", &[scratch.0.join("slides")]).expect("commit");

        let log = scratch.log();
        assert!(log.contains("slides/0001.md"), "{log}");
        assert!(!log.contains("notes.txt"), "{log}");
    }

    #[test]
    fn a_file_is_readable_as_head_has_it_rather_than_as_disk_has_it() {
        if !git_is_here() {
            return;
        }

        let scratch = Scratch::new("committed");
        let file = scratch.write("slides/0001.md", "# As committed\n");
        let repo = scratch.repo();
        repo.stage(&scratch.0.join("slides")).expect("stage");
        repo.commit("first", &[scratch.0.join("slides")]).expect("commit");

        scratch.write("slides/0001.md", "# As edited\n");

        assert_eq!(repo.committed(&file).as_deref(), Some("# As committed\n"));
        assert_eq!(repo.committed(&scratch.0.join("slides/0002.md")), None);
    }

    #[test]
    fn the_files_head_held_are_listed_in_deck_order() {
        if !git_is_here() {
            return;
        }

        // Sorted by name, the same rule the deck reader uses. Two orderings
        // would diff as a reorder nobody performed.
        let scratch = Scratch::new("ls-tree");
        scratch.write("slides/0002.md", "# Two\n");
        scratch.write("slides/0001.md", "# One\n");
        scratch.write("slides/notes.txt", "not a slide\n");
        let repo = scratch.repo();
        repo.stage(&scratch.0.join("slides")).expect("stage");
        repo.commit("first", &[scratch.0.join("slides")]).expect("commit");

        let files = repo.committed_files(&scratch.0.join("slides"));

        assert_eq!(files.len(), 2, "{files:?}");
        assert!(files[0].ends_with("0001.md"), "{files:?}");
        assert!(files[1].ends_with("0002.md"), "{files:?}");
    }

    #[test]
    fn a_commit_message_is_written_exactly_as_it_was_given() {
        if !git_is_here() {
            return;
        }

        // Nothing appends to it: no trailer, no footer, no attribution. The
        // message is the author's record of their own talk.
        let scratch = Scratch::new("message");
        scratch.write("slides/0001.md", "# One\n");
        let repo = scratch.repo();
        repo.stage(&scratch.0.join("slides")).expect("stage");
        repo.commit(
            "Add two slides on what goes wrong\n\n- added \"The fix\"\n",
            &[scratch.0.join("slides")],
        )
        .expect("commit");

        let message = run(&scratch.0, &["log", "-1", "--pretty=%B"]).expect("log");

        assert_eq!(message.trim_end(), "Add two slides on what goes wrong\n\n- added \"The fix\"");
    }

    #[test]
    fn a_status_record_is_read_as_its_code_and_its_path() {
        // Parsed without git, so the shapes that are awkward to produce on
        // demand — a rename, a path with a space — are still covered.
        let changes =
            parse_status("M  slides/0001.md\0?? slides/0002.md\0R  new name.md\0old name.md\0");

        assert_eq!(changes.len(), 3);
        assert_eq!(changes[0].status, "M ");
        assert!(changes[1].is_untracked());
        assert_eq!(changes[2].path, PathBuf::from("new name.md"));
    }

    #[test]
    fn a_path_is_spelled_with_forward_slashes_whatever_the_platform_uses() {
        // `git show HEAD:slides\0001.md` finds nothing on Windows.
        assert_eq!(slashed(&PathBuf::from("slides").join("0001.md")), "slides/0001.md");
    }

    #[test]
    fn the_repository_root_is_spelled_as_a_path_rather_than_as_nothing() {
        // The root is the empty relative path, and git reads an empty pathspec
        // as a fatal error rather than as everything.
        assert_eq!(path_arg(Path::new("")), ".");
        assert_eq!(slashed(Path::new("")), ".");
    }

    #[test]
    fn a_project_that_is_itself_the_repository_can_still_be_committed_whole() {
        if !git_is_here() {
            return;
        }

        // `slidx save --all` in a repository whose root is the project, which is
        // how every deck written by `slidx create` is laid out.
        let scratch = Scratch::new("whole");
        scratch.write("slides/0001.md", "# One\n");
        scratch.write("vite.config.ts", "export default {};\n");
        let repo = scratch.repo();

        assert_eq!(repo.changes(&scratch.0).expect("status").len(), 2);

        repo.stage(&scratch.0).expect("stage");
        repo.commit("everything", std::slice::from_ref(&scratch.0)).expect("commit");

        let log = scratch.log();
        assert!(log.contains("vite.config.ts"), "{log}");
        assert!(log.contains("slides/0001.md"), "{log}");
    }
}
