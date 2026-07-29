//! Fetching a release, and refusing to install one that does not check out.
//!
//! The same assets `install.sh` downloads, from the same release, verified
//! against the same `SHA256SUMS`. One publication, one format, two readers —
//! anything else would mean a version installed by the shell script and one
//! installed by `slidx version install` could differ and nobody would know.
//!
//! ## The check cannot be skipped here
//!
//! `install.sh` has to *look* for a hasher and refuse when it finds none,
//! because a shell script cannot compute a digest. This carries
//! [`crate::sha256`], so there is no detection, no fallback, and no branch that
//! installs without verifying. That is the one place the Rust side is strictly
//! better than the shell one, and it is worth having.
//!
//! What the check buys is the same either way, and worth being straight about:
//! the archive and its digest come from the same server, so it proves the
//! download arrived intact and is the file the release published — not that
//! the account publishing it was not compromised. The release also publishes a
//! Sigstore attestation for that, and `slidx version install` says so when it
//! finishes.
//!
//! ## Why `curl` rather than an HTTP client
//!
//! `std` has none, and every HTTP crate brings TLS with it — which for a binary
//! people are asked to pipe into a shell is a large tree to add for four
//! requests a year. `curl` is on macOS, on every Linux that has a package
//! manager, and has shipped in Windows since 2018; `wget` covers the rest.
//! Neither is asked to do anything but write a URL to a file.

use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::sha256;

const REPO: &str = "ubugeeei-prod/slidx";

/// The file listing every asset's digest. Named the same as in
/// `scripts/platforms.mjs`, because both readers build the URL from it.
pub const CHECKSUM_FILE: &str = "SHA256SUMS";

/// What went wrong, in the words of somebody who has to fix it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    NoDownloader,
    Unreachable {
        url: String,
    },
    /// The checksum file does not mention the asset. A release built before
    /// this platform existed looks exactly like this, and installing anyway
    /// would be verifying nothing while reporting success.
    NotListed {
        asset: String,
    },
    Mismatch {
        asset: String,
        published: String,
        got: String,
    },
    Unpacking {
        detail: String,
    },
}

impl Problem {
    pub fn message(&self) -> String {
        match self {
            Self::NoDownloader => "this needs curl or wget on PATH to fetch a release".to_string(),
            Self::Unreachable { url } => format!("could not download {url}"),
            Self::NotListed { asset } => {
                format!("{CHECKSUM_FILE} does not list {asset}, so it cannot be verified")
            }
            Self::Mismatch { asset, published, got } => format!(
                "checksum mismatch for {asset}\n  published {published}\n  got       {got}\n\
                 Nothing was installed."
            ),
            Self::Unpacking { detail } => format!("could not unpack the release: {detail}"),
        }
    }
}

/// Where a release's assets live.
///
/// `SLIDX_BASE_URL` overrides it, for an internal mirror or an air-gapped copy —
/// the same knob `install.sh` takes, spelled the same way, because somebody who
/// has set it up for one install channel should not have to discover that the
/// other one ignores it. The checksum is still checked: a mirror is exactly the
/// place a file is most likely to be stale.
pub fn release_url(version: &str) -> String {
    match std::env::var("SLIDX_BASE_URL") {
        Ok(base) if !base.is_empty() => base.trim_end_matches('/').to_string(),
        _ => format!("https://github.com/{REPO}/releases/download/v{version}"),
    }
}

/// The asset for one target triple.
pub fn asset_name(target: &str) -> String {
    if target.contains("windows") {
        format!("slidx-{target}.zip")
    } else {
        format!("slidx-{target}.tar.gz")
    }
}

/// The target triple this binary was built for.
///
/// Compiled in rather than detected, because the answer is a property of *this
/// executable* and not of the machine it happens to be running on. An x86-64
/// binary under Rosetta must fetch an x86-64 replacement, not the arm64 one the
/// machine would prefer — otherwise upgrading swaps a working slidx for one
/// that will not start.
pub const fn target() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-musl"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-unknown-linux-musl"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else {
        // A platform with no published binary. `install` says so and points at
        // `cargo install`, rather than 404-ing on a URL nobody can read.
        ""
    }
}

/// Finds the digest for one asset in a `SHA256SUMS` body.
///
/// The format is `<hex>  <name>`, with an optional `*` before the name for a
/// file hashed in binary mode. Absent means absent — never "assume it is fine".
pub fn published_digest(checksums: &str, asset: &str) -> Option<String> {
    checksums.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let digest = parts.next()?;
        let name = parts.next()?.trim_start_matches('*');

        (name == asset).then(|| digest.to_lowercase())
    })
}

/// Compares a file against its published digest.
pub fn verify(bytes: &[u8], checksums: &str, asset: &str) -> Result<(), Problem> {
    let published = published_digest(checksums, asset)
        .ok_or_else(|| Problem::NotListed { asset: asset.into() })?;

    let got = sha256::hex(bytes);

    if got == published {
        Ok(())
    } else {
        Err(Problem::Mismatch { asset: asset.into(), published, got })
    }
}

/// Downloads a URL to a file, using whichever fetcher is on the PATH.
pub fn fetch(url: &str, into: &Path) -> Result<(), Problem> {
    if let Some(parent) = into.parent() {
        let _ = fs::create_dir_all(parent);
    }

    for (program, arguments) in [
        (
            "curl",
            vec![
                "-fsSL".to_string(),
                url.to_string(),
                "-o".to_string(),
                into.display().to_string(),
            ],
        ),
        ("wget", vec!["-qO".to_string(), into.display().to_string(), url.to_string()]),
    ] {
        match Command::new(program).args(&arguments).stderr(Stdio::null()).status() {
            Ok(status) if status.success() => return Ok(()),
            // The program exists and failed: a 404, a proxy, no network. Say
            // so rather than falling through and blaming the next fetcher.
            Ok(_) => return Err(Problem::Unreachable { url: url.to_string() }),
            Err(_) => continue,
        }
    }

    Err(Problem::NoDownloader)
}

/// Unpacks an archive into a directory, using the tool that matches it.
pub fn unpack(archive: &Path, into: &Path) -> Result<(), Problem> {
    fs::create_dir_all(into).map_err(|error| Problem::Unpacking { detail: error.to_string() })?;

    let zip = archive.extension().is_some_and(|extension| extension == "zip");
    let (program, arguments) = if zip {
        (
            "unzip",
            vec![
                "-q".to_string(),
                "-o".to_string(),
                archive.display().to_string(),
                "-d".to_string(),
                into.display().to_string(),
            ],
        )
    } else {
        (
            "tar",
            vec![
                "-xzf".to_string(),
                archive.display().to_string(),
                "-C".to_string(),
                into.display().to_string(),
            ],
        )
    };

    match Command::new(program).args(&arguments).stderr(Stdio::null()).status() {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => {
            Err(Problem::Unpacking { detail: format!("{program} refused {}", archive.display()) })
        }
        Err(_) => Err(Problem::Unpacking { detail: format!("{program} is not on PATH") }),
    }
}

/// Marks a freshly unpacked binary executable.
///
/// tar carries the mode and a zip made on Windows does not, so this is set
/// rather than trusted — a binary that unpacked without its executable bit
/// fails with a permission error that names nothing useful.
pub fn make_executable(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))
    }

    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A checksum file in the format the release publishes.
    const SUMS: &str = "\
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  slidx-x86_64-unknown-linux-musl.tar.gz
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  slidx-aarch64-apple-darwin.tar.gz
";

    #[test]
    fn a_release_url_is_built_from_the_version_alone() {
        // No API call, so no rate limit and no token — the same reason
        // install.sh builds its URLs rather than asking what the assets are.
        assert_eq!(
            release_url("0.3.0"),
            "https://github.com/ubugeeei-prod/slidx/releases/download/v0.3.0"
        );
    }

    #[test]
    fn the_asset_is_named_after_the_target_triple() {
        assert_eq!(asset_name("aarch64-apple-darwin"), "slidx-aarch64-apple-darwin.tar.gz");
        assert_eq!(asset_name("x86_64-pc-windows-msvc"), "slidx-x86_64-pc-windows-msvc.zip");
    }

    #[test]
    fn the_target_is_the_one_this_binary_was_built_for_not_the_machines_preference() {
        // An x86-64 build under Rosetta has to fetch an x86-64 replacement.
        // Asking the machine would swap a working slidx for one that will not
        // start.
        let compiled = target();

        if cfg!(target_arch = "x86_64") {
            assert!(compiled.contains("x86_64"), "{compiled}");
        }
        if cfg!(target_arch = "aarch64") {
            assert!(compiled.contains("aarch64"), "{compiled}");
        }
    }

    #[test]
    fn a_published_digest_is_found_by_asset_name() {
        assert_eq!(
            published_digest(SUMS, "slidx-aarch64-apple-darwin.tar.gz").as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
    }

    #[test]
    fn a_binary_mode_star_before_the_name_is_not_part_of_the_name() {
        // `sha256sum -b` writes it, and a release built on a different runner
        // could carry it.
        let sums =
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad *slidx-x.tar.gz\n";

        assert!(published_digest(sums, "slidx-x.tar.gz").is_some());
    }

    #[test]
    fn an_asset_the_checksum_file_does_not_mention_is_a_failure_not_a_pass() {
        // A release built before this platform existed looks exactly like
        // this. Installing anyway would be verifying nothing while reporting
        // success.
        assert_eq!(published_digest(SUMS, "slidx-riscv64.tar.gz"), None);
        assert_eq!(
            verify(b"", SUMS, "slidx-riscv64.tar.gz"),
            Err(Problem::NotListed { asset: "slidx-riscv64.tar.gz".into() })
        );
    }

    #[test]
    fn an_archive_matching_its_published_digest_verifies() {
        // The empty digest is the first published SHA-256 test vector, which
        // is what makes this assertable without a fixture.
        assert_eq!(verify(b"", SUMS, "slidx-x86_64-unknown-linux-musl.tar.gz"), Ok(()));
    }

    #[test]
    fn an_archive_that_does_not_match_is_refused_and_says_both_digests() {
        // Somebody has to be able to tell a corrupted download from a swapped
        // asset, and the two numbers are how.
        let problem = verify(
            b"not the archive that was hashed",
            SUMS,
            "slidx-x86_64-unknown-linux-musl.tar.gz",
        )
        .expect_err("a mismatch");

        let message = problem.message();
        assert!(message.contains("checksum mismatch"), "{message}");
        assert!(message.contains("e3b0c442"), "{message}");
        assert!(message.contains("Nothing was installed"), "{message}");
    }

    #[test]
    fn a_digest_published_in_uppercase_still_matches() {
        let sums =
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855  slidx-x.tar.gz\n";

        assert_eq!(verify(b"", sums, "slidx-x.tar.gz"), Ok(()));
    }

    #[test]
    fn an_empty_checksum_file_verifies_nothing_rather_than_everything() {
        assert!(verify(b"anything", "", "slidx-x.tar.gz").is_err());
    }

    #[test]
    fn every_problem_says_something_somebody_could_act_on() {
        for problem in [
            Problem::NoDownloader,
            Problem::Unreachable { url: "https://example.com/x".into() },
            Problem::NotListed { asset: "x".into() },
            Problem::Mismatch { asset: "x".into(), published: "a".into(), got: "b".into() },
            Problem::Unpacking { detail: "tar".into() },
        ] {
            assert!(!problem.message().is_empty(), "{problem:?} says nothing");
        }
    }

    #[test]
    fn a_download_from_a_url_nothing_serves_reports_the_url() {
        // Not a network test: the point is that a fetcher which exists and
        // fails is reported as unreachable rather than as a missing fetcher.
        let into = std::env::temp_dir().join(format!("slidx-dl-{}", std::process::id()));
        let problem = fetch("file:///nowhere/at/all/slidx.tar.gz", &into);

        assert!(matches!(problem, Err(Problem::Unreachable { .. }) | Err(Problem::NoDownloader)));
        let _ = fs::remove_file(&into);
    }

    #[test]
    fn unpacking_something_that_is_not_an_archive_fails_rather_than_leaving_a_half_install() {
        let scratch = std::env::temp_dir().join(format!("slidx-unpack-{}", std::process::id()));
        let _ = fs::create_dir_all(&scratch);
        let archive = scratch.join("slidx-x.tar.gz");
        fs::write(&archive, "not an archive").expect("write");

        assert!(matches!(unpack(&archive, &scratch.join("out")), Err(Problem::Unpacking { .. })));

        let _ = fs::remove_dir_all(&scratch);
    }

    #[test]
    fn a_real_archive_round_trips_through_unpacking() {
        let scratch = std::env::temp_dir().join(format!("slidx-roundtrip-{}", std::process::id()));
        let _ = fs::remove_dir_all(&scratch);
        fs::create_dir_all(scratch.join("stage")).expect("stage");
        fs::write(scratch.join("stage/slidx"), "#!/bin/sh\necho hello\n").expect("write");

        let archive = scratch.join("slidx-x.tar.gz");
        let made = Command::new("tar")
            .args([
                "-czf",
                &archive.display().to_string(),
                "-C",
                &scratch.join("stage").display().to_string(),
                "slidx",
            ])
            .status();

        if made.map(|status| status.success()).unwrap_or(false) {
            let out = scratch.join("out");
            unpack(&archive, &out).expect("unpack");
            assert!(out.join("slidx").is_file());
        }

        let _ = fs::remove_dir_all(&scratch);
    }
}
