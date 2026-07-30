//! Which address on this machine a co-presenter can reach.
//!
//! A URL a co-presenter opens on the same Wi-Fi is what a conference actually
//! needs, and it involves no third party at all: no tunnel, no relay, no account,
//! and nothing that outlives the dev server. But printing such a URL means
//! knowing which of this machine's addresses the other laptops in the room can
//! route to, and `127.0.0.1` is not it.
//!
//! ## Why a UDP socket rather than a list of interfaces
//!
//! `std` cannot enumerate network interfaces, and every crate that can is a
//! platform matrix this binary has spent a lot of effort not having. What `std`
//! does have is the routing table, reachable indirectly: connecting a UDP socket
//! selects a route and binds a local address, and reading that address back is
//! the operating system answering "which of my addresses would I use to reach
//! there".
//!
//! Nothing is sent. `connect` on a UDP socket transmits no packet — it only
//! records a peer — so this touches the network no more than reading a file does,
//! and it works the same way on macOS, Linux and Windows.
//!
//! The peer is in `192.0.2.0/24`, which RFC 5737 reserves for documentation and
//! which therefore belongs to nobody. A real address here would be a machine
//! somebody owns appearing in a routing decision for no reason.

use std::net::{IpAddr, UdpSocket};

/// The documentation-only address used to ask the routing table a question.
const NOWHERE: &str = "192.0.2.1:9";

/// The address other machines on this network would reach this one at.
///
/// `None` on a machine with no route out — an aeroplane, an air-gapped room, a
/// container with no network. That is a real state and it is reported rather than
/// guessed at, because a share URL pointing at an address nothing can reach is
/// worse than being told there is no network.
pub fn on_this_network() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect(NOWHERE).ok()?;

    let address = socket.local_addr().ok()?.ip();

    // A machine with no route can still bind, and reports the unspecified
    // address when it does. Printing `0.0.0.0` in a URL would be a link nobody
    // can open.
    (!address.is_loopback() && !address.is_unspecified()).then_some(address)
}

/// The origin a browser would type, given an address and a port.
///
/// IPv6 goes in brackets. Without them the port reads as another group of the
/// address and the URL is silently a different one.
pub fn origin(address: IpAddr, port: u16) -> String {
    match address {
        IpAddr::V4(v4) => format!("http://{v4}:{port}"),
        IpAddr::V6(v6) => format!("http://[{v6}]:{port}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn an_ipv4_origin_is_what_a_browser_would_be_typed() {
        assert_eq!(
            origin(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 42)), 5173),
            "http://192.168.1.42:5173"
        );
    }

    #[test]
    fn an_ipv6_origin_brackets_the_address_so_the_port_is_still_a_port() {
        // Without brackets the `:5173` reads as another group of the address,
        // and the URL is quietly a different one.
        let address = IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1));

        assert_eq!(origin(address, 5173), "http://[fe80::1]:5173");
    }

    #[test]
    fn the_address_this_machine_is_reachable_at_is_never_loopback() {
        // The whole point: loopback is the address that does not work for the
        // person this URL is being handed to. On a machine with no route this
        // reports nothing, which is also correct.
        if let Some(address) = on_this_network() {
            assert!(!address.is_loopback(), "{address}");
            assert!(!address.is_unspecified(), "{address}");
        }
    }

    #[test]
    fn asking_the_routing_table_sends_nothing_and_can_be_asked_twice() {
        // `connect` on a UDP socket records a peer and transmits no packet, so
        // this is repeatable and costs the network nothing.
        assert_eq!(on_this_network(), on_this_network());
    }
}
