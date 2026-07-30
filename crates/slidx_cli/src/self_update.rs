//! `slidx self-update` — replace a managed binary with the latest release.
//!
//! The command uses the same archive, target selection and mandatory
//! `SHA256SUMS` verification as `slidx version install`. The only new decision
//! is which version to install: GitHub's `latest/download` redirect selects the
//! newest stable release, and the verified binary reports its own version
//! before anything is moved into the version store.
//!
//! Package managers remain in charge of binaries they installed. Quietly
//! putting a second slidx later on `PATH` would report success while leaving
//! the next invocation unchanged, which is worse than refusing with the exact
//! channel-specific update command.

use std::cmp::Ordering;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::home::Home;
use crate::report::{self, INDENT};
use crate::style::{Ink, Style};
use crate::version::current;
use crate::version::download;
use crate::version::provenance::Channel;
use crate::version::store::Store;
use crate::{Outcome, FOUND, MISUSE};

#[derive(Debug)]
struct Failure {
    code: u8,
    message: String,
}

impl Failure {
    fn could_not(message: impl Into<String>) -> Self {
        Self { code: MISUSE, message: message.into() }
    }

    fn refused(message: impl Into<String>) -> Self {
        Self { code: FOUND, message: message.into() }
    }

    fn outcome(self) -> Outcome {
        Outcome { stderr: format!("{}\n", self.message), code: self.code, ..Outcome::default() }
    }
}

/// A verified release waiting beside the version store.
#[derive(Debug)]
struct Staged {
    root: PathBuf,
    unpacked: PathBuf,
    version: String,
}

impl Drop for Staged {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Downloads, verifies and activates the newest stable release.
pub fn run(style: &Style) -> Outcome {
    let home = Home::discover();
    let store = Store::new(home.versions(), home.bin());
    let found = current::look(&home);

    if !may_manage(&found.channel) {
        return Outcome::misuse(format!(
            "slidx cannot self-update a binary owned by {}.\n\n{}\n",
            current::channel_name(&found.channel),
            found.channel.how_to_change()
        ));
    }

    if download::target().is_empty() {
        return Outcome::misuse(super::version::no_prebuilt_binary());
    }

    let staged = match stage(&store, &download::latest_release_url()) {
        Ok(staged) => staged,
        Err(failure) => return failure.outcome(),
    };

    match compare_versions(&staged.version, crate::version()) {
        Some(Ordering::Less) => {
            return Outcome::out(format!(
                "slidx {} is newer than the latest published release ({}).\n",
                crate::version(),
                staged.version
            ));
        }
        Some(Ordering::Equal) => {
            return Outcome::out(format!("slidx {} is already up to date.\n", crate::version()));
        }
        Some(Ordering::Greater) => {}
        None => {
            return Failure::refused(format!(
                "the release reported an invalid version `{}`",
                staged.version
            ))
            .outcome();
        }
    }

    let version = staged.version.clone();
    if let Err(error) = promote(&staged, &home, &store) {
        return Outcome::misuse(format!(
            "slidx {version} was verified but could not be activated: {error}\n"
        ));
    }

    let mut text =
        format!("{}\n\n", style.paint(Ink::Strong, format!("slidx {version} is now in use")));
    text.push_str(&report::flowed(
        "checksum verified against the release",
        INDENT,
        Ink::Pass,
        style,
    ));
    text.push_str(&report::flowed(
        &format!("previously {}", crate::version()),
        INDENT,
        Ink::Faint,
        style,
    ));

    Outcome::out(text)
}

fn may_manage(channel: &Channel) -> bool {
    matches!(channel, Channel::Managed { .. } | Channel::ShellInstall)
}

fn stage(store: &Store, base: &str) -> Result<Staged, Failure> {
    let root = store.root().join(format!(".self-update-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let unpacked = root.join("unpacked");
    let mut staged = Staged { root, unpacked, version: String::new() };

    let asset = download::asset_name(download::target());
    let archive = staged.root.join(&asset);
    let sums = staged.root.join(download::CHECKSUM_FILE);

    download::fetch(&format!("{base}/{asset}"), &archive)
        .map_err(|problem| Failure::could_not(problem.message()))?;
    download::fetch(&format!("{base}/{}", download::CHECKSUM_FILE), &sums)
        .map_err(|problem| Failure::could_not(problem.message()))?;

    let bytes = fs::read(&archive).map_err(|error| {
        Failure::could_not(format!("could not read the downloaded archive: {error}"))
    })?;
    let checksums = fs::read_to_string(&sums).map_err(|error| {
        Failure::could_not(format!("could not read the downloaded checksums: {error}"))
    })?;

    download::verify(&bytes, &checksums, &asset)
        .map_err(|problem| Failure::refused(problem.message()))?;
    download::unpack(&archive, &staged.unpacked)
        .map_err(|problem| Failure::could_not(problem.message()))?;

    let binary = staged.unpacked.join(crate::version::store::BINARY);
    download::make_executable(&binary).map_err(|error| {
        Failure::could_not(format!("could not make the release executable: {error}"))
    })?;
    staged.version = reported_version(&binary)?;

    Ok(staged)
}

fn reported_version(binary: &Path) -> Result<String, Failure> {
    let output = Command::new(binary).arg("--version").output().map_err(|error| {
        Failure::refused(format!("the verified release could not start: {error}"))
    })?;

    if !output.status.success() {
        return Err(Failure::refused(
            "the verified release did not answer `slidx --version` successfully",
        ));
    }

    parse_version_output(&output.stdout).ok_or_else(|| {
        Failure::refused("the verified release did not report a valid `slidx <version>` line")
    })
}

fn parse_version_output(output: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(output).ok()?.trim();
    let version = text.strip_prefix("slidx ")?.trim();
    valid_version(version).then(|| version.to_string())
}

fn valid_version(version: &str) -> bool {
    let core_end = version.find(['-', '+']).unwrap_or(version.len());
    let core = &version[..core_end];
    let mut numbers = core.split('.');
    let valid_core = (0..3).all(|_| {
        numbers.next().is_some_and(|part| {
            !part.is_empty() && part.bytes().all(|character| character.is_ascii_digit())
        })
    }) && numbers.next().is_none();

    valid_core
        && version[core_end..].bytes().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, b'.' | b'-' | b'+')
        })
}

fn compare_versions(left: &str, right: &str) -> Option<Ordering> {
    Some(precedence(left)?.cmp(&precedence(right)?))
}

fn precedence(version: &str) -> Option<(u64, u64, u64, bool)> {
    valid_version(version).then_some(())?;
    let core = version.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(str::parse::<u64>);
    let major = parts.next()?.ok()?;
    let minor = parts.next()?.ok()?;
    let patch = parts.next()?.ok()?;

    Some((major, minor, patch, !version.contains('-')))
}

fn promote(staged: &Staged, home: &Home, store: &Store) -> io::Result<()> {
    let destination = store.directory(&staged.version);
    if destination.exists() {
        fs::remove_dir_all(&destination)?;
    }

    fs::rename(&staged.unpacked, &destination)?;
    download::make_executable(&store.binary(&staged.version))?;
    crate::version::select(home, store, &staged.version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("slidx-self-update-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn only_a_managed_or_shell_installed_binary_can_take_itself_over() {
        assert!(may_manage(&Channel::Managed { version: "0.1.0".into() }));
        assert!(may_manage(&Channel::ShellInstall));
        assert!(!may_manage(&Channel::Npm));
        assert!(!may_manage(&Channel::Cargo));
        assert!(!may_manage(&Channel::Elsewhere));
    }

    #[test]
    fn a_release_reports_one_strict_safe_version_line() {
        assert_eq!(parse_version_output(b"slidx 0.2.0\n").as_deref(), Some("0.2.0"));
        assert_eq!(
            parse_version_output(b"slidx 1.0.0-rc.1+build.4\n").as_deref(),
            Some("1.0.0-rc.1+build.4")
        );
        assert!(parse_version_output(b"0.2.0\n").is_none());
        assert!(parse_version_output(b"slidx ../../outside\n").is_none());
        assert!(parse_version_output(b"slidx 0.2\n").is_none());
    }

    #[test]
    fn semantic_versions_compare_as_numbers_and_releases_follow_prereleases() {
        assert_eq!(compare_versions("0.10.0", "0.9.0"), Some(Ordering::Greater));
        assert_eq!(compare_versions("1.0.0", "1.0.0-rc.1"), Some(Ordering::Greater));
        assert_eq!(compare_versions("0.2.0", "0.2.0"), Some(Ordering::Equal));
    }

    #[test]
    fn promoting_a_verified_stage_installs_selects_and_records_it() {
        let scratch = Scratch::new("promote");
        let home = Home::at(scratch.0.join("home"));
        let store = Store::new(home.versions(), home.bin());
        let root = store.root().join(".self-update-test");
        let unpacked = root.join("unpacked");
        fs::create_dir_all(&unpacked).expect("unpacked");
        fs::write(unpacked.join(crate::version::store::BINARY), b"verified").expect("binary");
        let staged = Staged { root, unpacked, version: "0.2.0".into() };

        promote(&staged, &home, &store).expect("promote");

        assert!(store.is_installed("0.2.0"));
        assert!(store.shim().is_file());
        assert_eq!(crate::version::pin::read(&home.default_version()).as_deref(), Some("0.2.0"));
    }

    #[cfg(unix)]
    #[test]
    fn the_downloaded_archive_is_verified_unpacked_and_asked_its_version() {
        use std::os::unix::fs::PermissionsExt;

        let scratch = Scratch::new("stage");
        let release = scratch.0.join("release");
        let payload = scratch.0.join("payload");
        fs::create_dir_all(&release).expect("release");
        fs::create_dir_all(&payload).expect("payload");

        let binary = payload.join(crate::version::store::BINARY);
        fs::write(&binary, "#!/bin/sh\nprintf 'slidx 0.2.0\\n'\n").expect("fixture binary");
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).expect("executable");

        let asset = download::asset_name(download::target());
        let archive = release.join(&asset);
        let status = Command::new("tar")
            .args(["-czf"])
            .arg(&archive)
            .arg("-C")
            .arg(&payload)
            .arg(crate::version::store::BINARY)
            .status()
            .expect("tar");
        assert!(status.success());

        let bytes = fs::read(&archive).expect("archive");
        fs::write(
            release.join(download::CHECKSUM_FILE),
            format!("{}  {asset}\n", crate::sha256::hex(&bytes)),
        )
        .expect("checksums");

        let home = Home::at(scratch.0.join("home"));
        let store = Store::new(home.versions(), home.bin());
        let staged =
            stage(&store, &format!("file://{}", release.display())).expect("verified stage");

        assert_eq!(staged.version, "0.2.0");
        assert_eq!(
            fs::read_to_string(staged.unpacked.join(crate::version::store::BINARY))
                .expect("unpacked binary"),
            "#!/bin/sh\nprintf 'slidx 0.2.0\\n'\n"
        );
    }
}
