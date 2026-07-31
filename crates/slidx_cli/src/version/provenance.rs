//! Which slidx is actually running, and who put it there.
//!
//! This module exists because of one specific hour that people lose. You run
//! `slidx version use 0.3.0`, it says it worked, and `slidx --version` still
//! prints the old number — because `npm i -g slidx` put a different binary
//! earlier on your PATH months ago and nothing has ever mentioned it. Every fix
//! you then apply is applied to the wrong binary.
//!
//! A version manager that cannot tell you it is not in charge is worse than no
//! version manager, because it invites you to trust it. So the answer here is
//! not "the version is X" — it is:
//!
//! - the file that is *running*, resolved through symlinks;
//! - which install channel that file implies;
//! - **whether the version manager is in charge of it at all**;
//! - every other `slidx` on the PATH, and which one would win.
//!
//! ## Everything is a pure function of readings
//!
//! [`Reading`] is a struct of paths and strings; nothing here calls the
//! operating system. That is the same split [`slidx_doctor`] is built on and
//! for the same reason: the interesting cases are other people's machines —
//! npm shadowing a managed install, a Homebrew slidx from two years ago, a
//! `cargo install` in a container — and none of them are reachable from the one
//! machine the tests happen to run on.

use std::path::{Path, PathBuf};

/// Where a running binary came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Channel {
    /// `~/.slidx/versions/<version>/slidx` — the version manager is in charge.
    Managed { version: String },
    /// `~/.slidx/bin/slidx` as a real file: `install.sh` put it there and
    /// nothing has been managed since.
    ShellInstall,
    /// Inside a `node_modules`, so npm, pnpm or yarn owns it.
    Npm,
    /// `~/.cargo/bin` — built from source with `cargo install`.
    Cargo,
    /// A package manager's own prefix. Updating it is that manager's job.
    System { manager: &'static str },
    /// Somewhere else entirely. Named rather than guessed at.
    Elsewhere,
}

impl Channel {
    /// True when `slidx version use` can change what runs.
    ///
    /// The single most useful bit in the whole report: everything else is
    /// detail, and this is the thing that decides whether the next command
    /// somebody types will do anything at all.
    pub fn is_managed(&self) -> bool {
        matches!(self, Self::Managed { .. })
    }

    /// What to do to change versions, given where this one came from.
    pub fn how_to_change(&self) -> &'static str {
        match self {
            Self::Managed { .. } => "slidx version use <version>",
            Self::ShellInstall => {
                "this binary was put here by install.sh and is not managed. \
                 `slidx version install <version>` takes over from it"
            }
            Self::Npm => {
                "npm owns this one: `npm i -g slidx@<version>`, or remove it and \
                 let the version manager's bin directory come first on PATH"
            }
            Self::Cargo => "cargo owns this one: `cargo install slidx_cli --version <version>`",
            Self::System { .. } => "your package manager owns this one",
            Self::Elsewhere => "nothing slidx knows about put this here",
        }
    }
}

/// What the machine says.
#[derive(Debug, Clone, Default)]
pub struct Reading {
    /// `std::env::current_exe`, already resolved through symlinks.
    pub exe: Option<PathBuf>,
    /// `~/.slidx`.
    pub home: PathBuf,
    /// Every directory on PATH, in order.
    pub path: Vec<PathBuf>,
    /// Files named `slidx` found on PATH, in the order they would be tried.
    pub on_path: Vec<PathBuf>,
}

/// Everything that can be said about the running binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub exe: Option<PathBuf>,
    pub channel: Channel,
    /// True when `~/.slidx/bin` is on the PATH at all.
    pub bin_on_path: bool,
    /// Another `slidx` that comes before the managed one and would win.
    pub shadowed_by: Option<PathBuf>,
}

/// Reads a set of observations into an answer.
pub fn of(reading: &Reading) -> Provenance {
    let versions = reading.home.join("versions");
    let bin = reading.home.join("bin");

    let channel = reading
        .exe
        .as_deref()
        .map(|exe| classify(exe, &versions, &bin))
        .unwrap_or(Channel::Elsewhere);

    Provenance {
        exe: reading.exe.clone(),
        bin_on_path: reading.path.iter().any(|entry| entry == &bin),
        shadowed_by: shadowed_by(&reading.on_path, &bin),
        channel,
    }
}

fn classify(exe: &Path, versions: &Path, bin: &Path) -> Channel {
    if let Ok(rest) = exe.strip_prefix(versions) {
        // `versions/<v>/slidx` — the first component is the version.
        if let Some(version) = rest.components().next() {
            return Channel::Managed {
                version: version.as_os_str().to_string_lossy().into_owned(),
            };
        }
    }

    if exe.starts_with(bin) {
        // Reached `bin/` without resolving into `versions/`, so it is a real
        // file rather than the version manager's link: install.sh put it here.
        return Channel::ShellInstall;
    }

    if exe.components().any(|part| part.as_os_str() == "node_modules") {
        return Channel::Npm;
    }

    let text = exe.to_string_lossy();

    if text.contains("/.cargo/bin/") || text.contains("\\.cargo\\bin\\") {
        return Channel::Cargo;
    }

    for (needle, manager) in
        [("/Cellar/", "Homebrew"), ("/homebrew/", "Homebrew"), ("/nix/store/", "Nix")]
    {
        if text.contains(needle) {
            return Channel::System { manager };
        }
    }

    Channel::Elsewhere
}

/// The `slidx` that would win, when it is not the managed one.
///
/// Only reported when the managed binary is on the PATH *and* something else
/// comes first — that combination is the trap. One slidx on the PATH is not a
/// problem however it was installed.
fn shadowed_by(on_path: &[PathBuf], bin: &Path) -> Option<PathBuf> {
    let managed = on_path.iter().position(|found| found.starts_with(bin))?;
    let first = on_path.first()?;

    (managed > 0).then(|| first.clone())
}

/// Every `slidx` on a PATH, in the order a shell would try them.
///
/// Takes the variable rather than reading it, and takes the "does this file
/// exist" test as a closure, so a PATH that is not this machine's can be
/// examined.
pub fn on_path(path: &str, windows: bool, exists: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let separator = if windows { ';' } else { ':' };
    let names: &[&str] = if windows { &["slidx.exe", "slidx.cmd"] } else { &["slidx"] };

    path.split(separator)
        .filter(|entry| !entry.is_empty())
        .flat_map(|entry| names.iter().map(move |name| Path::new(entry).join(name)))
        .filter(|candidate| exists(candidate))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/home/somebody/.slidx";

    fn reading(exe: &str) -> Reading {
        Reading { exe: Some(PathBuf::from(exe)), home: PathBuf::from(HOME), ..Reading::default() }
    }

    fn channel_of(exe: &str) -> Channel {
        of(&reading(exe)).channel
    }

    #[test]
    fn a_binary_under_the_versions_directory_is_managed_and_names_its_version() {
        // The one case where `slidx version use` will do what somebody expects.
        assert_eq!(
            channel_of("/home/somebody/.slidx/versions/0.3.0/slidx"),
            Channel::Managed { version: "0.3.0".into() }
        );
    }

    #[test]
    fn a_real_file_in_the_bin_directory_is_the_shell_installer_and_not_managed() {
        // install.sh writes here directly. The version manager has never been
        // involved, and saying "managed" would be a lie that costs an hour.
        let channel = channel_of("/home/somebody/.slidx/bin/slidx");

        assert_eq!(channel, Channel::ShellInstall);
        assert!(!channel.is_managed());
    }

    #[test]
    fn a_binary_inside_node_modules_belongs_to_npm() {
        assert_eq!(
            channel_of("/usr/local/lib/node_modules/@slidxjs/cli-linux-x64/bin/slidx"),
            Channel::Npm
        );
    }

    #[test]
    fn a_binary_in_cargos_bin_directory_was_built_from_source() {
        assert_eq!(channel_of("/home/somebody/.cargo/bin/slidx"), Channel::Cargo);
        assert_eq!(channel_of(r"C:\Users\somebody\.cargo\bin\slidx.exe"), Channel::Cargo);
    }

    #[test]
    fn a_package_managers_own_prefix_is_named_rather_than_guessed_at() {
        assert_eq!(
            channel_of("/opt/homebrew/Cellar/slidx/0.1.0/bin/slidx"),
            Channel::System { manager: "Homebrew" }
        );
        assert_eq!(
            channel_of("/nix/store/abc123-slidx/bin/slidx"),
            Channel::System { manager: "Nix" }
        );
    }

    #[test]
    fn anywhere_else_is_reported_as_elsewhere_rather_than_assumed_managed() {
        // The safe default. Claiming to manage something you do not is the
        // failure this whole module is here to prevent.
        assert_eq!(channel_of("/opt/tools/slidx"), Channel::Elsewhere);
        assert!(!channel_of("/opt/tools/slidx").is_managed());
    }

    #[test]
    fn a_binary_that_cannot_be_located_at_all_is_elsewhere_rather_than_a_panic() {
        // `current_exe` can fail on an unusual platform or a deleted binary.
        let provenance = of(&Reading { home: PathBuf::from(HOME), ..Reading::default() });

        assert_eq!(provenance.channel, Channel::Elsewhere);
        assert_eq!(provenance.exe, None);
    }

    #[test]
    fn every_channel_says_what_to_do_to_change_versions() {
        // A report that names a problem and no next action is noise — the same
        // rule the doctor's findings hold to.
        for channel in [
            Channel::Managed { version: "0.1.0".into() },
            Channel::ShellInstall,
            Channel::Npm,
            Channel::Cargo,
            Channel::System { manager: "Homebrew" },
            Channel::Elsewhere,
        ] {
            assert!(!channel.how_to_change().is_empty(), "{channel:?} says nothing");
        }
    }

    #[test]
    fn an_unmanaged_channel_says_plainly_that_the_version_manager_is_not_in_charge() {
        assert!(Channel::ShellInstall.how_to_change().contains("not managed"));
        assert!(Channel::Npm.how_to_change().contains("npm owns this one"));
    }

    #[test]
    fn an_npm_slidx_earlier_on_the_path_is_reported_as_shadowing_the_managed_one() {
        // The hour this module exists to give back. `version use` succeeds,
        // `slidx --version` does not change, and nothing ever says why.
        let npm = PathBuf::from("/usr/local/bin/slidx");
        let managed = PathBuf::from("/home/somebody/.slidx/bin/slidx");

        let provenance = of(&Reading {
            exe: Some(npm.clone()),
            home: PathBuf::from(HOME),
            on_path: vec![npm.clone(), managed],
            ..Reading::default()
        });

        assert_eq!(provenance.shadowed_by, Some(npm));
    }

    #[test]
    fn nothing_is_shadowed_when_the_managed_binary_comes_first() {
        let managed = PathBuf::from("/home/somebody/.slidx/bin/slidx");
        let other = PathBuf::from("/usr/local/bin/slidx");

        let provenance = of(&Reading {
            exe: Some(managed.clone()),
            home: PathBuf::from(HOME),
            on_path: vec![managed, other],
            ..Reading::default()
        });

        assert_eq!(provenance.shadowed_by, None);
    }

    #[test]
    fn one_slidx_on_the_path_is_never_reported_as_shadowed_however_it_was_installed() {
        // Not a problem. Reporting it would train people to ignore the warning
        // that matters.
        let provenance = of(&Reading {
            exe: Some(PathBuf::from("/usr/local/bin/slidx")),
            home: PathBuf::from(HOME),
            on_path: vec![PathBuf::from("/usr/local/bin/slidx")],
            ..Reading::default()
        });

        assert_eq!(provenance.shadowed_by, None);
    }

    #[test]
    fn whether_the_managed_bin_directory_is_on_the_path_at_all_is_reported() {
        // The other half of the same trap: `version use` writes a link into a
        // directory no shell ever looks in.
        let with = of(&Reading {
            home: PathBuf::from(HOME),
            path: vec![PathBuf::from("/usr/bin"), PathBuf::from("/home/somebody/.slidx/bin")],
            ..Reading::default()
        });
        let without = of(&Reading {
            home: PathBuf::from(HOME),
            path: vec![PathBuf::from("/usr/bin")],
            ..Reading::default()
        });

        assert!(with.bin_on_path);
        assert!(!without.bin_on_path);
    }

    #[test]
    fn every_slidx_on_a_unix_path_is_found_in_the_order_a_shell_would_try_them() {
        let found = on_path("/usr/local/bin:/home/somebody/.slidx/bin:/usr/bin", false, |path| {
            path != Path::new("/usr/bin/slidx")
        });

        assert_eq!(
            found,
            [
                PathBuf::from("/usr/local/bin/slidx"),
                PathBuf::from("/home/somebody/.slidx/bin/slidx"),
            ]
        );
    }

    #[test]
    fn a_windows_path_is_split_on_semicolons_and_looks_for_the_executable_extensions() {
        // Asserted on the pieces rather than on whole paths: `Path::join` uses
        // the separator of the machine running the test, and this case is about
        // a Windows PATH examined from anywhere.
        let found = on_path(r"C:\tools;C:\Users\somebody\.slidx\bin", true, |path| {
            path.extension().is_some_and(|extension| extension == "exe")
        });

        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|path| path.file_name().is_some_and(|name| name == "slidx.exe")));
        assert!(found[0].to_string_lossy().contains(r"C:\tools"));
        assert!(found[1].to_string_lossy().contains(r".slidx\bin"));
    }

    #[test]
    fn an_empty_entry_on_the_path_is_skipped_rather_than_read_as_the_root() {
        // A trailing colon is common and means nothing. Reading it as `/` would
        // have this reporting `/slidx`.
        assert!(on_path("/usr/bin::", false, |_| true)
            .iter()
            .all(|found| found.starts_with("/usr")));
    }
}
