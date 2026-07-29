//! Installing and switching versions, against a release served from disk.
//!
//! The unit tests cover the pieces; this covers the thing somebody actually
//! does — install a version, switch to it, and have the shim run it. A release
//! is built in a scratch directory and served over `file://`, so the whole
//! download-verify-unpack-select path runs with nothing mocked out.
//!
//! The case worth having most is the one that must *not* work: an archive whose
//! checksum does not match has to leave the machine exactly as it found it.
//!
//! Driven through the binary rather than the library, because the environment
//! is half the behaviour here — `SLIDX_HOME` and `SLIDX_BASE_URL` are read from
//! the process, and a test that set them in-process would fight every other
//! test running beside it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use slidx_cli::version::store::BINARY;

/// A scratch machine: its own `~/.slidx`, and its own release to install from.
struct Machine {
    root: PathBuf,
}

impl Machine {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "slidx-vm-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("scratch");

        Self { root }
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    fn release(&self) -> PathBuf {
        self.root.join("release")
    }

    /// The binary one version installed, under the name this platform uses.
    fn installed(&self, version: &str) -> PathBuf {
        self.home().join("versions").join(version).join(BINARY)
    }

    /// The path a shell resolves `slidx` to.
    fn shim(&self) -> PathBuf {
        self.home().join("bin").join(BINARY)
    }

    /// Publishes one version: an archive, and a checksum file describing it.
    ///
    /// `corrupt` rewrites the archive after the digest is taken, which is what
    /// a truncated download or a swapped asset looks like from here.
    fn publish(&self, version: &str, corrupt: bool) {
        let release = self.release();
        fs::create_dir_all(&release).expect("release");

        let stage = self.root.join("stage");
        let _ = fs::remove_dir_all(&stage);
        fs::create_dir_all(&stage).expect("stage");
        fs::write(stage.join(BINARY), format!("#!/bin/sh\necho \"slidx {version}\"\n"))
            .expect("binary");

        // The same format the release publishes for this target: a zip on
        // Windows, a gzipped tar everywhere else. Publishing a tarball a
        // Windows build would never ask for would test a path nobody runs.
        let asset = asset_name();
        let archive = release.join(&asset).display().to_string();
        let from = stage.display().to_string();
        let arguments: Vec<&str> = if asset.ends_with(".zip") {
            vec!["--format=zip", "-cf", &archive, "-C", &from, BINARY]
        } else {
            vec!["-czf", &archive, "-C", &from, BINARY]
        };

        let made = Command::new("tar").args(&arguments).status().expect("tar");
        assert!(made.success(), "could not build the release archive");

        let digest = sha256_of(&release.join(&asset));
        fs::write(release.join("SHA256SUMS"), format!("{digest}  {asset}\n")).expect("sums");

        if corrupt {
            fs::write(release.join(&asset), "not the archive that was hashed").expect("corrupt");
        }
    }

    /// Runs the binary with this machine's environment.
    fn run(&self, arguments: &[&str]) -> Output {
        let output = Command::new(binary())
            .args(arguments)
            .env("SLIDX_HOME", self.home())
            .env("SLIDX_BASE_URL", file_url(&self.release()))
            .env("NO_COLOR", "1")
            .output()
            .expect("run slidx");

        Output {
            code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct Output {
    code: i32,
    stdout: String,
    stderr: String,
}

/// The asset name for the target this test binary was built for.
///
/// Asked of the code under test rather than spelled out again, so the fixture
/// cannot publish an asset the installer would never ask for.
fn asset_name() -> String {
    slidx_cli::version::download::asset_name(slidx_cli::version::download::target())
}

/// A `file://` URL for a directory on this machine.
///
/// A Windows path is not a URL: `C:\a\b` has to be spelled `/C:/a/b`, so the
/// drive letter needs a third slash and the separators have to be turned round.
/// Interpolating the path as it comes gives a fetcher something it cannot read.
fn file_url(path: &Path) -> String {
    let text = path.display().to_string().replace('\\', "/").replace(' ', "%20");

    if text.starts_with('/') {
        format!("file://{text}")
    } else {
        format!("file:///{text}")
    }
}

fn binary() -> PathBuf {
    // `cargo test` puts integration binaries next to the ones they test.
    let mut path = std::env::current_exe().expect("test binary");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join(BINARY)
}

fn sha256_of(path: &Path) -> String {
    slidx_cli::sha256::hex(&fs::read(path).expect("read"))
}

/// Skipped where there is no published target for this machine, and where the
/// binary under test has not been built yet.
fn runnable() -> bool {
    !slidx_cli::version::download::target().is_empty() && binary().is_file()
}

#[test]
fn installing_a_version_verifies_it_and_puts_it_where_use_can_find_it() {
    if !runnable() {
        return;
    }

    let machine = Machine::new("install");
    machine.publish("0.3.0", false);

    let output = machine.run(&["version", "install", "0.3.0"]);

    assert_eq!(output.code, 0, "{}{}", output.stdout, output.stderr);
    assert!(output.stdout.contains("checksum verified"), "{}", output.stdout);
    assert!(machine.installed("0.3.0").is_file());
}

#[test]
fn an_archive_that_does_not_match_its_published_digest_installs_nothing() {
    if !runnable() {
        return;
    }

    // The property the whole verification path exists for: a corrupted or
    // swapped download leaves the machine exactly as it found it.
    let machine = Machine::new("corrupt");
    machine.publish("0.3.0", true);

    let output = machine.run(&["version", "install", "0.3.0"]);

    assert_ne!(output.code, 0);
    assert!(output.stderr.contains("checksum mismatch"), "{}", output.stderr);
    assert!(output.stderr.contains("Nothing was installed"), "{}", output.stderr);
    assert!(!machine.home().join("versions/0.3.0").exists(), "a version directory was left behind");
}

#[test]
fn a_release_whose_checksum_file_does_not_mention_the_asset_installs_nothing() {
    if !runnable() {
        return;
    }

    // What an installer pointed at a release built before this platform
    // existed would otherwise do: verify nothing and report success.
    let machine = Machine::new("unlisted");
    machine.publish("0.3.0", false);
    fs::write(machine.release().join("SHA256SUMS"), "0000  something-else.tar.gz\n").expect("sums");

    let output = machine.run(&["version", "install", "0.3.0"]);

    assert_ne!(output.code, 0);
    assert!(output.stderr.contains("cannot be verified"), "{}", output.stderr);
    assert!(!machine.home().join("versions/0.3.0").exists());
}

#[test]
fn using_a_version_points_the_shim_at_it_and_records_the_choice() {
    if !runnable() {
        return;
    }

    let machine = Machine::new("use");
    machine.publish("0.3.0", false);
    machine.run(&["version", "install", "0.3.0"]);

    let output = machine.run(&["version", "use", "0.3.0"]);

    assert_eq!(output.code, 0, "{}{}", output.stdout, output.stderr);

    // Both halves: the link the shell finds, and the record that survives it
    // being clobbered.
    let shim = machine.shim();
    assert!(shim.exists(), "no shim");
    assert!(fs::read_to_string(&shim).expect("read through the shim").contains("0.3.0"));
    assert_eq!(fs::read_to_string(machine.home().join("version")).expect("record").trim(), "0.3.0");
}

#[test]
fn install_and_switch_in_one_step_when_asked() {
    if !runnable() {
        return;
    }

    let machine = Machine::new("install-use");
    machine.publish("0.3.0", false);

    let output = machine.run(&["version", "install", "0.3.0", "--use"]);

    assert_eq!(output.code, 0, "{}{}", output.stdout, output.stderr);
    assert!(output.stdout.contains("now in use"), "{}", output.stdout);
    assert!(machine.shim().exists());
}

#[test]
fn listing_marks_the_version_in_use() {
    if !runnable() {
        return;
    }

    let machine = Machine::new("list");
    machine.publish("0.3.0", false);
    machine.run(&["version", "install", "0.3.0", "--use"]);

    let output = machine.run(&["version", "list"]);

    assert_eq!(output.code, 0);
    assert!(output.stdout.contains("0.3.0"), "{}", output.stdout);
    assert!(output.stdout.contains("default"), "{}", output.stdout);
}

#[test]
fn listing_nothing_says_how_to_install_something() {
    if !runnable() {
        return;
    }

    let machine = Machine::new("list-empty");
    let output = machine.run(&["version", "list"]);

    assert_eq!(output.code, 0);
    assert!(output.stdout.contains("slidx version install"), "{}", output.stdout);
}

#[test]
fn using_a_version_that_is_not_installed_says_how_to_install_it() {
    if !runnable() {
        return;
    }

    let machine = Machine::new("use-missing");
    let output = machine.run(&["version", "use", "9.9.9"]);

    assert_ne!(output.code, 0);
    assert!(output.stderr.contains("slidx version install 9.9.9"), "{}", output.stderr);
    assert!(!machine.shim().exists(), "a dead shim was left behind");
}

#[test]
fn removing_the_version_in_use_is_refused_rather_than_leaving_a_dead_shim() {
    if !runnable() {
        return;
    }

    // A `slidx` on the PATH that reports "command not found" from a directory
    // that exists is a genuinely baffling thing to be handed.
    let machine = Machine::new("remove-in-use");
    machine.publish("0.3.0", false);
    machine.run(&["version", "install", "0.3.0", "--use"]);

    let output = machine.run(&["version", "remove", "0.3.0"]);

    assert_ne!(output.code, 0);
    assert!(output.stderr.contains("pointing at nothing"), "{}", output.stderr);
    assert!(machine.installed("0.3.0").is_file());
}

#[test]
fn a_project_pin_is_reported_by_current_along_with_the_file_it_came_from() {
    if !runnable() {
        return;
    }

    let machine = Machine::new("pin");
    let project = machine.root.join("talks/vueconf/slides");
    fs::create_dir_all(&project).expect("project");
    fs::write(machine.root.join("talks/vueconf/.slidx-version"), "0.9.9\n").expect("pin");

    let output = Command::new(binary())
        .args(["version", "current"])
        .current_dir(&project)
        .env("SLIDX_HOME", machine.home())
        .env("NO_COLOR", "1")
        .output()
        .expect("run");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Found by walking up from a directory two levels below it — the only
    // behaviour that makes a pin worth having.
    assert!(stdout.contains("0.9.9"), "{stdout}");
    assert!(stdout.contains(".slidx-version"), "{stdout}");
    assert!(stdout.contains("not installed"), "{stdout}");
}

#[test]
fn current_reports_a_binary_that_the_version_manager_did_not_install() {
    if !runnable() {
        return;
    }

    // The test binary is a cargo build under target/, which is exactly the
    // "nothing slidx knows about put this here" case — and it must say so
    // rather than claiming to manage it.
    let machine = Machine::new("unmanaged");
    let output = machine.run(&["version", "current"]);

    assert_eq!(output.code, 0);
    assert!(output.stdout.contains("not managed"), "{}", output.stdout);
    assert!(output.stdout.contains("will not change what runs"), "{}", output.stdout);
}

#[test]
fn current_reports_as_json_for_a_setup_script_to_read() {
    if !runnable() {
        return;
    }

    let machine = Machine::new("json");
    let output = machine.run(&["version", "current", "--json"]);

    assert_eq!(output.code, 0);
    assert!(output.stdout.contains("\"managed\""), "{}", output.stdout);
    assert!(output.stdout.contains("\"channel\""), "{}", output.stdout);
}
