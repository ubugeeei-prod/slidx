//! Reading the machine.
//!
//! The only module in the crate that talks to the operating system, and it
//! produces exactly one thing: an [`Environment`] for the checks to read. If
//! this module were absent, every check would still work — it would simply have
//! to be handed its readings by somebody else, which is precisely what the test
//! suite does.
//!
//! Three rules hold here, and all three exist because this code runs ninety
//! seconds before a talk:
//!
//! **Nothing blocks.** Every reading has a deadline. A subprocess that hangs is
//! killed, a socket that stalls times out, and the result is
//! [`Reading::unavailable`] rather than a doctor that never prints.
//!
//! **Nothing panics.** A reading that cannot be taken has somewhere to say so,
//! so there is never a reason to unwrap. The module is compiled with the
//! panicking shortcuts denied.
//!
//! **Nothing guesses.** Where a platform genuinely cannot answer, the reading is
//! unavailable and the check reports `Unknown`. A plausible default here would
//! turn into a green line about something nobody measured.
//!
//! Readings are taken concurrently. Run one after another they would sum to
//! something like ten seconds of subprocess startup, and a pre-flight nobody
//! has time to wait for is a pre-flight nobody runs.

pub mod camera;
pub mod clock;
pub mod command;
pub mod disk;
pub mod fonts;
pub mod network;
pub mod power;
pub mod processes;

use std::env;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::thread::{self, ScopedJoinHandle};
use std::time::Duration;

use crate::environment::{Environment, Expectation, Reading};

/// Where to look, what to compare against, and how long to wait.
#[derive(Debug, Clone)]
pub struct Request {
    /// The directory a recording or an export would be written to. Its volume
    /// is the one measured — the deck may well live on an external drive.
    pub workspace: PathBuf,
    /// What the deck and the booking say. Passed through to the checks
    /// untouched; the probe never invents an expectation.
    pub expected: Expectation,
    /// Ceiling for any one reading. Readings run concurrently, so this is
    /// roughly the ceiling for the whole run as well.
    pub timeout: Duration,
    /// Dialled to see whether the network is up. `None` makes the probe take
    /// no network reading at all, for an embedder that would rather not have a
    /// diagnostic tool open sockets.
    pub network_target: Option<NetworkTarget>,
    /// NTP server used to measure clock skew. `None` skips it, and the check
    /// reports `Unknown` — which is the honest answer, since without a
    /// reference clock there is no way to know.
    pub time_server: Option<String>,
}

/// What the network reading dials.
///
/// An address and a hostname rather than one or the other, because the two
/// answer different questions: the address tests whether packets leave the
/// building, the hostname tests whether anything resolves. A captive portal
/// passes the first and fails the second, and that combination is the signal
/// that the venue wifi wants a login.
#[derive(Debug, Clone)]
pub struct NetworkTarget {
    pub addr: SocketAddr,
    pub hostname: String,
}

impl Default for NetworkTarget {
    fn default() -> Self {
        Self {
            // An IP literal, so the TCP result does not quietly depend on DNS.
            addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(1, 1, 1, 1), 443)),
            // Reserved by IANA for exactly this, and so cannot be taken over.
            hostname: "example.com".to_string(),
        }
    }
}

impl Default for Request {
    /// Two seconds per reading: long enough for a subprocess to start on a cold
    /// machine, short enough that a speaker does not give up and start anyway.
    fn default() -> Self {
        Self {
            workspace: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            expected: Expectation::default(),
            timeout: Duration::from_secs(2),
            network_target: Some(NetworkTarget::default()),
            time_server: Some("pool.ntp.org:123".to_string()),
        }
    }
}

impl Request {
    /// A request that opens no sockets.
    ///
    /// For an embedder that would rather a diagnostic tool did not reach the
    /// network. The two checks that depend on it then report `Unknown` and say
    /// why, which is the truthful outcome rather than a degraded one.
    pub fn offline() -> Self {
        Self { network_target: None, time_server: None, ..Self::default() }
    }

    pub fn in_workspace(mut self, workspace: impl Into<PathBuf>) -> Self {
        self.workspace = workspace.into();
        self
    }

    pub fn expecting(mut self, expected: Expectation) -> Self {
        self.expected = expected;
        self
    }

    pub fn within(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// Takes every reading this machine will give up.
///
/// Never fails, never blocks past the request's timeout, and never panics: a
/// reading that cannot be taken comes back as unavailable with the reason.
pub fn read(request: &Request) -> Environment {
    thread::scope(|scope| {
        let power = scope.spawn(|| power::read(request.timeout));
        let disk = scope.spawn(|| disk::read(&request.workspace, request.timeout));
        let clock = scope.spawn(|| clock::read_zone(request.timeout));
        let skew =
            scope.spawn(|| clock::read_skew(request.time_server.as_deref(), request.timeout));
        let fonts = scope.spawn(|| fonts::read(request.timeout));
        let processes = scope.spawn(|| processes::read(request.timeout));
        let cameras = scope.spawn(|| camera::read(request.timeout));
        let network =
            scope.spawn(|| network::read(request.network_target.as_ref(), request.timeout));

        Environment {
            power: joined(power),
            disk: joined(disk),
            clock: joined(clock),
            skew: joined(skew),
            fonts: joined(fonts),
            processes: joined(processes),
            cameras: joined(cameras),
            network: joined(network),
            expected: request.expected.clone(),
        }
    })
}

/// Collects one reading, treating a panicking probe as an unavailable reading.
///
/// A probe should not panic and is written not to. This is the seatbelt: one
/// misbehaving reading must not take the other six down with it, because a
/// partial report at a lectern beats no report.
fn joined<T>(handle: ScopedJoinHandle<'_, Reading<T>>) -> Reading<T> {
    handle.join().unwrap_or_else(|_| Reading::unavailable("this probe failed while running"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_request_measures_the_directory_the_deck_is_in() {
        // The deck may well be on an external drive; measuring the boot volume
        // would answer a question nobody asked.
        assert!(!Request::default().workspace.as_os_str().is_empty());
    }

    #[test]
    fn an_offline_request_opens_no_sockets() {
        // An embedder must be able to say "do not touch the network", and get
        // an honest unknown rather than a silent network call.
        let request = Request::offline();

        assert!(request.network_target.is_none());
        assert!(request.time_server.is_none());
    }

    #[test]
    fn the_network_target_is_dialled_by_address_and_resolved_by_name() {
        // Two different questions. Using a hostname for both would make an
        // unreachable network and a broken resolver look identical.
        let target = NetworkTarget::default();

        assert!(target.addr.is_ipv4());
        assert!(!target.hostname.is_empty());
    }

    #[test]
    fn reading_a_real_machine_fills_every_field_one_way_or_the_other() {
        // The one test that touches the operating system. It cannot assert
        // what this machine's battery says — that is the whole reason the
        // checks take injected readings — only that the probe answers for
        // every field, within its deadline, without panicking.
        let request = Request::offline().within(Duration::from_secs(5));
        let environment = read(&request);

        // Nothing to assert about the values; that they exist is the contract.
        let _ = format!("{environment:?}");
    }

    #[test]
    fn a_real_read_stays_inside_a_generous_deadline() {
        // Readings run concurrently, so the whole run is bounded by the
        // slowest one rather than by their sum. A pre-flight that takes ten
        // seconds is one a speaker abandons.
        let started = std::time::Instant::now();
        let _ = read(&Request::offline().within(Duration::from_secs(2)));

        assert!(started.elapsed() < Duration::from_secs(20), "took {:?}", started.elapsed());
    }
}
