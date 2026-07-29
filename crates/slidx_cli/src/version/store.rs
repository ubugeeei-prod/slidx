//! The versions on disk, and which one `bin/` points at.
//!
//! ```text
//! ~/.slidx/
//!   versions/0.2.0/slidx
//!   versions/0.3.0/slidx
//!   bin/slidx           -> ../versions/0.3.0/slidx
//!   version             0.3.0
//! ```
//!
//! ## Why both a link and a file
//!
//! `bin/slidx` is what the shell finds; `version` is the record of what was
//! chosen. They look redundant and are not. The link is a fact about this
//! machine's filesystem — it can be replaced by an installer, clobbered by a
//! package manager, or lost when somebody copies their dotfiles to a new
//! laptop. The file is a statement of intent that survives all of that, and it
//! is what [`super::provenance`] compares the running binary against to notice
//! that the two have come apart.
//!
//! ## Relative links
//!
//! `bin/slidx` points at `../versions/0.3.0/slidx`, not at an absolute path.
//! `~/.slidx` moved, copied, or mounted somewhere else still works — and the
//! whole directory being portable is what makes `SLIDX_HOME` a real option
//! rather than a footgun.
//!
//! On Windows the binary is copied instead. Symlinks there need either
//! developer mode or an administrator, and a version manager that demands
//! elevation is one nobody runs.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The file inside a version directory.
pub const BINARY: &str = if cfg!(windows) { "slidx.exe" } else { "slidx" };

/// Installed versions.
#[derive(Debug, Clone)]
pub struct Store {
    versions: PathBuf,
    bin: PathBuf,
}

impl Store {
    pub fn new(versions: impl Into<PathBuf>, bin: impl Into<PathBuf>) -> Self {
        Self { versions: versions.into(), bin: bin.into() }
    }

    pub fn root(&self) -> &Path {
        &self.versions
    }

    /// Where one version lives, installed or not.
    pub fn directory(&self, version: &str) -> PathBuf {
        self.versions.join(version)
    }

    pub fn binary(&self, version: &str) -> PathBuf {
        self.directory(version).join(BINARY)
    }

    /// The path a shell resolves `slidx` to.
    pub fn shim(&self) -> PathBuf {
        self.bin.join(BINARY)
    }

    /// True when the binary is actually there.
    ///
    /// A directory without one is a half-finished install — an interrupted
    /// download, a full disk — and treating it as installed would have `use`
    /// point the shim at nothing.
    pub fn is_installed(&self, version: &str) -> bool {
        self.binary(version).is_file()
    }

    /// Every installed version, newest first.
    pub fn installed(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.versions) else {
            return Vec::new();
        };

        let mut versions: Vec<String> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| self.is_installed(name))
            .collect();

        versions.sort_by_key(|version| std::cmp::Reverse(order(version)));
        versions
    }

    /// Points the shim at a version.
    ///
    /// Replaces whatever was there, including a real binary left by
    /// `install.sh` — taking over from it is the point, and refusing would
    /// leave somebody with a version manager that cannot manage the one slidx
    /// they have.
    pub fn select(&self, version: &str) -> io::Result<()> {
        if !self.is_installed(version) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{version} is not installed"),
            ));
        }

        fs::create_dir_all(&self.bin)?;
        let shim = self.shim();

        // Removed first: a symlink cannot be created over an existing entry,
        // and on Windows a running binary cannot be overwritten in place.
        if shim.exists() || shim.symlink_metadata().is_ok() {
            fs::remove_file(&shim)?;
        }

        link(&self.relative_target(version), &shim)
    }

    /// `../versions/<version>/slidx`, so the whole home directory can move.
    fn relative_target(&self, version: &str) -> PathBuf {
        Path::new("..").join("versions").join(version).join(BINARY)
    }

    /// Removes a version, and says so rather than silently doing nothing.
    pub fn remove(&self, version: &str) -> io::Result<()> {
        if !self.directory(version).exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{version} is not installed"),
            ));
        }

        fs::remove_dir_all(self.directory(version))
    }
}

/// Symlink on Unix, copy on Windows.
#[cfg(unix)]
fn link(target: &Path, shim: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, shim)
}

#[cfg(not(unix))]
fn link(target: &Path, shim: &Path) -> io::Result<()> {
    // Resolved against the shim's own directory, because a relative symlink
    // target is relative to the link and a copy source is relative to the
    // process's working directory.
    let source = shim.parent().unwrap_or(Path::new(".")).join(target);
    fs::copy(source, shim).map(|_| ())
}

/// A version as numbers, so `0.10.0` sorts above `0.9.0`.
///
/// A string sort would put `0.10.0` before `0.9.0` and quietly offer somebody
/// the wrong "latest". Anything unparseable sorts last rather than first: a
/// directory somebody made by hand should not become the newest version.
fn order(version: &str) -> (u64, u64, u64, bool) {
    let core = version.split(['-', '+']).next().unwrap_or(version);
    let mut parts = core.split('.').map(|part| part.parse::<u64>().unwrap_or(0));

    let numbers = (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0));

    // A pre-release sorts below the release it precedes.
    (numbers.0, numbers.1, numbers.2, !version.contains('-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-store-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");
            Self(path)
        }

        fn store(&self) -> Store {
            Store::new(self.0.join("versions"), self.0.join("bin"))
        }

        /// Installs a version the way a download would leave it.
        fn install(&self, version: &str) {
            let store = self.store();
            fs::create_dir_all(store.directory(version)).expect("a version directory");
            fs::write(store.binary(version), format!("#!/bin/sh\necho slidx {version}\n"))
                .expect("a binary");
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn nothing_installed_lists_nothing_rather_than_failing() {
        // Every machine starts here.
        assert!(Store::new("/nowhere/versions", "/nowhere/bin").installed().is_empty());
    }

    #[test]
    fn installed_versions_are_listed_newest_first() {
        let scratch = Scratch::new("order");
        for version in ["0.1.0", "0.3.0", "0.2.0"] {
            scratch.install(version);
        }

        assert_eq!(scratch.store().installed(), ["0.3.0", "0.2.0", "0.1.0"]);
    }

    #[test]
    fn ten_sorts_above_nine_rather_than_below_it() {
        // A string sort would offer somebody 0.9.0 as the newest they have.
        let scratch = Scratch::new("ten");
        for version in ["0.9.0", "0.10.0", "0.10.1"] {
            scratch.install(version);
        }

        assert_eq!(scratch.store().installed(), ["0.10.1", "0.10.0", "0.9.0"]);
    }

    #[test]
    fn a_pre_release_sorts_below_the_release_it_precedes() {
        let scratch = Scratch::new("pre");
        for version in ["0.3.0", "0.3.0-rc.1"] {
            scratch.install(version);
        }

        assert_eq!(scratch.store().installed(), ["0.3.0", "0.3.0-rc.1"]);
    }

    #[test]
    fn a_directory_nobody_could_parse_sorts_last_rather_than_becoming_the_newest() {
        let scratch = Scratch::new("junk");
        scratch.install("0.2.0");
        scratch.install("whatever-this-is");

        assert_eq!(scratch.store().installed()[0], "0.2.0");
    }

    #[test]
    fn a_directory_with_no_binary_in_it_is_not_an_installed_version() {
        // A half-finished install: an interrupted download, a full disk.
        // Counting it would have `use` point the shim at nothing.
        let scratch = Scratch::new("empty");
        scratch.install("0.2.0");
        fs::create_dir_all(scratch.store().directory("0.3.0")).expect("a directory");

        assert_eq!(scratch.store().installed(), ["0.2.0"]);
        assert!(!scratch.store().is_installed("0.3.0"));
    }

    #[test]
    fn selecting_a_version_puts_a_working_slidx_where_the_shell_will_find_it() {
        let scratch = Scratch::new("select");
        scratch.install("0.3.0");
        let store = scratch.store();

        store.select("0.3.0").expect("select");

        assert!(store.shim().exists());
        assert_eq!(
            fs::read_to_string(store.shim()).expect("read through the shim"),
            "#!/bin/sh\necho slidx 0.3.0\n"
        );
    }

    #[test]
    fn selecting_again_replaces_the_previous_choice() {
        // Symlinks cannot be created over an existing entry, so the second
        // `use` is the one that would fail if this were not handled.
        let scratch = Scratch::new("reselect");
        scratch.install("0.2.0");
        scratch.install("0.3.0");
        let store = scratch.store();

        store.select("0.2.0").expect("first");
        store.select("0.3.0").expect("second");

        assert!(fs::read_to_string(store.shim()).expect("read").contains("0.3.0"));
    }

    #[test]
    fn selecting_replaces_a_real_binary_the_shell_installer_left_behind() {
        // Taking over from install.sh is the point. Refusing would leave
        // somebody with a version manager that cannot manage the slidx they
        // actually have.
        let scratch = Scratch::new("takeover");
        scratch.install("0.3.0");
        let store = scratch.store();
        fs::create_dir_all(store.shim().parent().expect("bin")).expect("bin");
        fs::write(store.shim(), "an unmanaged binary").expect("write");

        store.select("0.3.0").expect("select");

        assert!(fs::read_to_string(store.shim()).expect("read").contains("0.3.0"));
    }

    #[test]
    fn selecting_something_that_is_not_installed_fails_rather_than_leaving_a_dead_link() {
        // A shim pointing at nothing is a `slidx` that reports "command not
        // found" from a directory that is on the PATH.
        let scratch = Scratch::new("missing");

        assert!(scratch.store().select("9.9.9").is_err());
        assert!(!scratch.store().shim().exists());
    }

    #[cfg(unix)]
    #[test]
    fn the_shim_points_at_a_relative_path_so_the_home_directory_can_move() {
        // A copied dotfiles directory, a mounted volume, SLIDX_HOME pointing
        // somewhere else. An absolute link would break in all three.
        let scratch = Scratch::new("relative");
        scratch.install("0.3.0");
        let store = scratch.store();
        store.select("0.3.0").expect("select");

        let target = fs::read_link(store.shim()).expect("a symlink");

        assert!(target.is_relative(), "{target:?}");
        assert_eq!(target, Path::new("../versions/0.3.0/slidx"));
    }

    #[test]
    fn removing_a_version_takes_the_whole_directory() {
        let scratch = Scratch::new("remove");
        scratch.install("0.2.0");
        let store = scratch.store();

        store.remove("0.2.0").expect("remove");

        assert!(store.installed().is_empty());
        assert!(!store.directory("0.2.0").exists());
    }

    #[test]
    fn removing_something_that_is_not_there_says_so_rather_than_reporting_success() {
        assert!(Scratch::new("remove-missing").store().remove("9.9.9").is_err());
    }
}
