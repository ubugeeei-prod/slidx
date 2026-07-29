//! Where slidx keeps its own things on this machine.
//!
//! ```text
//! ~/.slidx/
//!   config.toml     settings, when there are any
//!   index.json      the decks this machine knows about
//!   versions/       installed slidx versions
//!   bin/            the shim that is on PATH
//! ```
//!
//! ## Agreeing with the installer
//!
//! `install.sh` resolves this same directory, in the same order, and puts the
//! binary in the same `bin/`. That is not a coincidence to be maintained by
//! hand — it is the whole reason the version manager can ever be in charge of
//! the binary somebody is running. Two tools with two ideas of where slidx
//! lives is the failure the layout exists to avoid: a version manager that is
//! silently not managing the binary on your PATH is worse than none, because
//! you debug the wrong thing for an hour.
//!
//! The order is `SLIDX_HOME`, then `XDG_DATA_HOME/slidx`, then the platform
//! default. A test asserts the shell script spells it the same way.
//!
//! ## Windows gets a Windows path
//!
//! `%LOCALAPPDATA%\slidx`, not a dotfile in the user profile. A dot-prefixed
//! directory is a Unix convention for hiding things and hides nothing on
//! Windows — it is just a folder with an odd name in the middle of somebody's
//! home directory. `XDG_DATA_HOME` is not consulted there either, for the same
//! reason: it is not a convention that platform has.
//!
//! ## Nothing here creates anything
//!
//! Resolving a path is not the same as needing it to exist. `slidx lint` in a
//! read-only container should lint, not fail because it could not make a
//! directory for an index nobody asked for. Directories are created by the
//! code that writes into them, at the point it writes.

use std::env;
use std::path::{Path, PathBuf};

/// The directory holding slidx's own state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Home {
    root: PathBuf,
}

impl Home {
    /// Reads the environment. Called once, near the top of a command.
    pub fn discover() -> Self {
        Self::from_env(&Env::read())
    }

    /// An explicit root, for a test or an embedder.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Resolves the root from a set of readings, so every branch is reachable
    /// from a test without setting process-wide variables that parallel tests
    /// would fight over.
    pub fn from_env(env: &Env) -> Self {
        if let Some(explicit) = non_empty(&env.slidx_home) {
            return Self::at(explicit);
        }

        // XDG is a Unix convention. Honouring it on Windows would put slidx
        // somewhere no other Windows tool looks.
        if !env.windows {
            if let Some(data) = non_empty(&env.xdg_data_home) {
                return Self::at(Path::new(data).join("slidx"));
            }
        }

        let base = if env.windows {
            non_empty(&env.local_app_data).or_else(|| non_empty(&env.user_profile))
        } else {
            non_empty(&env.home)
        };

        // A machine with no home directory at all is a container running as a
        // user with no passwd entry. Falling back to the working directory
        // keeps every command working; nothing here is state anybody would
        // miss if it landed in the wrong place.
        let base = base.unwrap_or(".");

        Self::at(Path::new(base).join(if env.windows { "slidx" } else { ".slidx" }))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Settings.
    ///
    /// Nothing reads or writes this yet, and it is declared rather than created
    /// for that reason: an empty config file invites somebody to put settings
    /// in it that nothing will ever read. The global default slidx version is
    /// [`Self::default_version`] instead — the same one-line format as a
    /// project's `.slidx-version`, so there is one thing to learn rather than
    /// two.
    pub fn config(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    /// The decks this machine knows about.
    pub fn index(&self) -> PathBuf {
        self.root.join("index.json")
    }

    /// Installed slidx versions, one directory each.
    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }

    /// The version to use when no `.slidx-version` is found.
    pub fn default_version(&self) -> PathBuf {
        self.root.join("version")
    }

    /// The directory that goes on PATH. The same one `install.sh` writes to.
    pub fn bin(&self) -> PathBuf {
        self.root.join("bin")
    }
}

/// The environment readings the root depends on.
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub slidx_home: Option<String>,
    pub xdg_data_home: Option<String>,
    pub home: Option<String>,
    pub local_app_data: Option<String>,
    pub user_profile: Option<String>,
    pub windows: bool,
}

impl Env {
    pub fn read() -> Self {
        Self {
            slidx_home: env::var("SLIDX_HOME").ok(),
            xdg_data_home: env::var("XDG_DATA_HOME").ok(),
            home: env::var("HOME").ok(),
            local_app_data: env::var("LOCALAPPDATA").ok(),
            user_profile: env::var("USERPROFILE").ok(),
            windows: cfg!(windows),
        }
    }
}

/// An exported-but-empty variable is how one looks after a shell script unset
/// it badly. Treating it as a request to install into `/slidx` would be a
/// surprising way to find that out.
fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().filter(|text| !text.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unix() -> Env {
        Env { home: Some("/home/somebody".into()), ..Env::default() }
    }

    fn windows() -> Env {
        Env {
            local_app_data: Some(r"C:\Users\somebody\AppData\Local".into()),
            user_profile: Some(r"C:\Users\somebody".into()),
            windows: true,
            ..Env::default()
        }
    }

    #[test]
    fn a_unix_machine_gets_a_dotfile_in_the_home_directory() {
        assert_eq!(Home::from_env(&unix()).root(), Path::new("/home/somebody/.slidx"));
    }

    #[test]
    fn xdg_data_home_wins_over_the_default_where_the_platform_has_that_convention() {
        let env = Env { xdg_data_home: Some("/home/somebody/.local/share".into()), ..unix() };

        assert_eq!(Home::from_env(&env).root(), Path::new("/home/somebody/.local/share/slidx"));
    }

    #[test]
    fn slidx_home_wins_over_everything() {
        // The escape hatch, and the one `install.sh` documents. It has to
        // outrank the conventions or it is not one.
        let env = Env {
            slidx_home: Some("/opt/slidx".into()),
            xdg_data_home: Some("/home/somebody/.local/share".into()),
            ..unix()
        };

        assert_eq!(Home::from_env(&env).root(), Path::new("/opt/slidx"));
    }

    #[test]
    fn an_exported_but_empty_variable_is_not_a_request_to_install_into_the_root() {
        // How a variable looks after a shell script unset it badly. `/slidx`
        // would be a surprising place to find out.
        let env =
            Env { slidx_home: Some(String::new()), xdg_data_home: Some(String::new()), ..unix() };

        assert_eq!(Home::from_env(&env).root(), Path::new("/home/somebody/.slidx"));
    }

    #[test]
    fn windows_gets_a_windows_path_rather_than_a_dotfile() {
        // A dot-prefixed directory hides nothing on Windows. It is a folder
        // with an odd name sitting in the middle of somebody's home directory.
        assert_eq!(
            Home::from_env(&windows()).root(),
            Path::new(r"C:\Users\somebody\AppData\Local").join("slidx")
        );
    }

    #[test]
    fn windows_does_not_consult_xdg_data_home() {
        // Not a convention that platform has. Honouring it would put slidx
        // somewhere no other Windows tool looks.
        let env = Env { xdg_data_home: Some("/wherever".into()), ..windows() };

        assert!(Home::from_env(&env).root().starts_with(r"C:\Users\somebody\AppData\Local"));
    }

    #[test]
    fn windows_without_localappdata_falls_back_to_the_user_profile() {
        let env = Env { local_app_data: None, ..windows() };

        assert_eq!(Home::from_env(&env).root(), Path::new(r"C:\Users\somebody").join("slidx"));
    }

    #[test]
    fn a_machine_with_no_home_directory_still_resolves_to_something() {
        // A container running as a user with no passwd entry. Every command
        // has to keep working; none of this is state anybody would miss.
        assert_eq!(Home::from_env(&Env::default()).root(), Path::new("./.slidx"));
    }

    #[test]
    fn every_path_hangs_off_the_one_root() {
        // So pointing SLIDX_HOME somewhere moves all of it, rather than
        // scattering half of slidx across two directories.
        let home = Home::at("/tmp/slidx");

        for path in
            [home.config(), home.index(), home.versions(), home.bin(), home.default_version()]
        {
            assert!(path.starts_with("/tmp/slidx"), "{path:?} escaped the root");
        }
    }

    #[test]
    fn the_binary_directory_is_the_one_the_shell_installer_writes_to() {
        // If these two ever disagree, the version manager is not managing the
        // binary anybody is running.
        assert_eq!(Home::at("/home/somebody/.slidx").bin(), Path::new("/home/somebody/.slidx/bin"));
    }

    #[test]
    fn resolving_a_path_creates_nothing() {
        // `slidx lint` in a read-only container has to lint, not fail making a
        // directory for an index nobody asked for.
        let home = Home::at("/tmp/slidx-should-not-exist-from-resolving");
        let _ = home.index();
        let _ = home.versions();

        assert!(!home.root().exists());
    }
}
