//! Is there a network, and does anything resolve.
//!
//! Two separate questions, asked separately on purpose. A captive portal — the
//! venue wifi that wants you to agree to its terms — completes a TCP handshake
//! and then refuses to resolve a hostname, so a probe that asked only one
//! question would report "the network is fine" or "there is no network", and
//! both would send the speaker to fix the wrong thing.
//!
//! Name resolution is the part that can hang: [`std::net::ToSocketAddrs`] has
//! no timeout, and a venue resolver that has stopped answering will sit there
//! for the length of the system's own retry schedule. So it runs on its own
//! thread behind a channel deadline, and if it overruns the thread is abandoned
//! rather than waited for.

use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::environment::{Network, Reading};
use crate::probe::NetworkTarget;

pub fn read(target: Option<&NetworkTarget>, timeout: Duration) -> Reading<Network> {
    let Some(target) = target else {
        return Reading::unavailable("slidx was asked not to touch the network");
    };

    let started = Instant::now();
    let connected = TcpStream::connect_timeout(&target.addr, timeout).is_ok();
    let round_trip = started.elapsed();

    Reading::known(Network {
        tcp_reachable: connected,
        dns_resolves: resolves(&target.hostname, timeout),
        // Only meaningful when something answered. A timeout's elapsed time
        // measures the timeout, not the network.
        round_trip_ms: connected.then(|| round_trip.as_millis().min(u128::from(u64::MAX)) as u64),
        target: target.addr.to_string(),
    })
}

/// Whether a hostname resolves, within the deadline.
///
/// Port zero: nothing is dialled, so this asks the resolver and nothing else.
fn resolves(hostname: &str, timeout: Duration) -> bool {
    let (sender, receiver) = mpsc::channel();
    let query = format!("{hostname}:0");

    thread::spawn(move || {
        let resolved = query.to_socket_addrs().map(|mut addrs| addrs.next().is_some());
        let _ = sender.send(resolved.unwrap_or(false));
    });

    // A resolver that overruns is treated as one that did not answer, which is
    // exactly what a speaker would conclude from watching a browser spin.
    receiver.recv_timeout(timeout).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

    /// An address in the documentation range, which by definition routes
    /// nowhere — so this is a reliable "no network" without depending on the
    /// machine actually being offline.
    fn unroutable() -> NetworkTarget {
        NetworkTarget {
            addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 443)),
            hostname: "slidx-no-such-host.invalid".to_string(),
        }
    }

    #[test]
    fn a_request_that_declines_the_network_takes_no_reading() {
        // An embedder must be able to say "do not open sockets" and get an
        // honest unavailable rather than a silent connection.
        let reading = read(None, Duration::from_millis(50));

        assert!(!reading.is_known());
        assert!(reading.reason().unwrap().contains("not to touch the network"));
    }

    #[test]
    fn an_unreachable_target_reports_a_reading_rather_than_an_error() {
        // "There is no network" is a fact about the venue, and the check has
        // something to say about it. It is not a failed measurement.
        let reading = read(Some(&unroutable()), Duration::from_millis(200));
        let network = reading.value().unwrap();

        assert!(!network.tcp_reachable);
        assert!(!network.dns_resolves);
    }

    #[test]
    fn an_unreachable_target_quotes_no_round_trip_time() {
        // The elapsed time of a failed connection measures our own timeout,
        // and printing it as a latency would be a fabricated number.
        let reading = read(Some(&unroutable()), Duration::from_millis(200));

        assert_eq!(reading.value().unwrap().round_trip_ms, None);
    }

    #[test]
    fn an_unreachable_target_gives_up_inside_its_deadline() {
        // The venue case. A pre-flight that stalls for thirty seconds on a
        // dead network is one nobody waits for.
        let started = Instant::now();
        let _ = read(Some(&unroutable()), Duration::from_millis(200));

        assert!(started.elapsed() < Duration::from_secs(10), "took {:?}", started.elapsed());
    }

    #[test]
    fn the_reading_names_what_was_dialled() {
        // A speaker reading "no route to 1.1.1.1:443" can judge the claim; one
        // reading "no route" cannot.
        let reading = read(Some(&unroutable()), Duration::from_millis(200));

        assert_eq!(reading.value().unwrap().target, "192.0.2.1:443");
    }

    #[test]
    fn a_hostname_that_cannot_resolve_is_false_rather_than_a_hang() {
        let started = Instant::now();

        assert!(!resolves("slidx-no-such-host.invalid", Duration::from_millis(500)));
        assert!(started.elapsed() < Duration::from_secs(10), "took {:?}", started.elapsed());
    }

    #[test]
    fn localhost_resolves_without_a_network() {
        // Proves the resolver path works at all, on a name that is answerable
        // from the machine's own hosts file with the cable pulled out.
        assert!(resolves("localhost", Duration::from_secs(5)));
    }
}
