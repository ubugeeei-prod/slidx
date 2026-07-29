//! Network reachability, which is not a requirement.
//!
//! **This check can never fail.** A slidx deck is a static bundle: it renders,
//! animates and navigates with the network cable pulled out, and that is a
//! design guarantee rather than a happy accident. A venue with no working wifi
//! is the normal case, not the broken one, and a pre-flight that goes red about
//! it would be training speakers to walk on stage having already dismissed a
//! red line.
//!
//! So the line exists to answer one question a speaker actually has — *will my
//! live demo work?* — and to say, in the same breath, that the deck itself does
//! not care.

use crate::environment::{Environment, Network};
use crate::finding::Finding;

const ID: &str = "network";

/// Said in every non-passing remedy here. The reassurance is the point: a
/// speaker who reads this line at a venue with no wifi should stop worrying
/// about it in one sentence.
const NOT_NEEDED: &str = "you do not need this — the deck renders, animates and navigates offline";

pub fn check(environment: &Environment) -> Finding {
    let Some(network) = environment.network.value() else {
        return Finding::unknown(
            ID,
            format!(
                "network reachability was not measured: {}",
                environment.network.reason().unwrap_or("no reason given")
            ),
            format!("{NOT_NEEDED}. Check it by hand only if part of your talk goes online"),
        );
    };

    let target = &network.target;

    match (network.tcp_reachable, network.dns_resolves) {
        (true, true) => Finding::pass(ID, describe_reachable(network)),
        // A captive portal completes the handshake and then refuses to resolve
        // anything until you have clicked through its terms — which is a very
        // different thing to tell a speaker than "there is no network".
        (true, false) => Finding::warn(
            ID,
            format!(
                "{target} answers but nothing resolves — a captive portal, or a broken resolver"
            ),
            format!(
                "{NOT_NEEDED}. If a demo goes online, open a browser now and sign in to the venue \
                 network before you start, not in front of the room"
            ),
        ),
        (false, _) => Finding::warn(
            ID,
            format!("no route to {target}"),
            format!(
                "{NOT_NEEDED}. If a demo goes online, record a fallback of it now, or tether to \
                 your phone before you start"
            ),
        ),
    }
}

fn describe_reachable(network: &Network) -> String {
    match network.round_trip_ms {
        Some(round_trip) => format!("{} reachable in {round_trip}ms", network.target),
        None => format!("{} reachable", network.target),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::Reading;
    use crate::finding::Status;

    fn on(network: Network) -> Environment {
        Environment::new().with_network(Reading::known(network))
    }

    #[test]
    fn a_working_connection_passes() {
        let finding = check(&on(Network::reachable("1.1.1.1:443", 24)));

        assert_eq!(finding.status, Status::Pass);
        assert!(finding.detail.contains("24ms"), "got: {}", finding.detail);
    }

    #[test]
    fn no_network_at_all_never_fails() {
        // The guarantee this module exists to hold. A deck works offline, so an
        // offline venue is not a fault and must not be reported as one.
        assert_eq!(check(&on(Network::offline("1.1.1.1:443"))).status, Status::Warn);
    }

    #[test]
    fn no_combination_of_readings_can_produce_a_failure() {
        // Exhaustive over the two signals, because "never fails" is a promise
        // and not a tendency.
        for tcp_reachable in [true, false] {
            for dns_resolves in [true, false] {
                let network = Network {
                    tcp_reachable,
                    dns_resolves,
                    round_trip_ms: None,
                    target: "1.1.1.1:443".into(),
                };

                assert_ne!(
                    check(&on(network)).status,
                    Status::Fail,
                    "tcp={tcp_reachable} dns={dns_resolves} produced a failure"
                );
            }
        }
    }

    #[test]
    fn an_unmeasured_network_never_fails_either() {
        let environment = Environment::new().with_network(Reading::unavailable("no socket"));

        assert_eq!(check(&environment).status, Status::Unknown);
    }

    #[test]
    fn every_non_passing_remedy_says_the_deck_does_not_need_the_network() {
        // The sentence that stops a speaker worrying about the one line on the
        // report they cannot do anything about.
        let environments = [
            on(Network::offline("1.1.1.1:443")),
            on(Network::captive("1.1.1.1:443", 12)),
            Environment::new().with_network(Reading::unavailable("no socket")),
        ];

        for environment in environments {
            let finding = check(&environment);
            assert!(
                finding.remedy.as_deref().unwrap().contains("you do not need this"),
                "got: {finding:?}"
            );
        }
    }

    #[test]
    fn a_captive_portal_is_distinguished_from_having_no_network() {
        // One is fixed by clicking through a login page, the other is not.
        // Telling a speaker "no network" when the wifi wants a login sends
        // them to fix the wrong thing.
        let finding = check(&on(Network::captive("1.1.1.1:443", 12)));

        assert!(finding.detail.contains("captive portal"), "got: {}", finding.detail);
        assert!(finding.remedy.unwrap().contains("sign in"));
    }

    #[test]
    fn an_offline_venue_is_told_to_record_a_fallback_of_any_live_demo() {
        // The only genuinely useful action, and it has to happen before the
        // talk rather than during it.
        let finding = check(&on(Network::offline("1.1.1.1:443")));

        assert!(finding.remedy.unwrap().contains("fallback"));
    }

    #[test]
    fn a_reachable_target_without_a_timing_still_reads_sensibly() {
        let network = Network {
            tcp_reachable: true,
            dns_resolves: true,
            round_trip_ms: None,
            target: "1.1.1.1:443".into(),
        };

        assert_eq!(check(&on(network)).detail, "1.1.1.1:443 reachable");
    }
}
