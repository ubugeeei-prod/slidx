//! The promises the whole suite makes, rather than any one check.
//!
//! Each check tests its own boundaries next to the reasoning for them. What is
//! left is the set of claims that only make sense across all of them at once,
//! and that a new check could quietly break: that nothing red is ever left
//! without a next action, that an unavailable reading never reports green, and
//! that the report is ordered the way it is read.
//!
//! The environment is injected, so these run over thousands of machines that do
//! not exist — which is the only way to make claims about *other people's*
//! laptops rather than about the one the tests happen to run on.

use slidx_doctor::environment::{
    Audio, Clock, Disk, Display, Displays, InstalledFonts, Network, Notifications, Power,
    RunningProcesses, Skew,
};
use slidx_doctor::{check, probe, Environment, Expectation, Platform, Reading, Status};

const GIB: u64 = 1024 * 1024 * 1024;

/// Every platform the readings can come from, including the one that is none of
/// them. Three checks build a remedy out of this, so it is a dimension of the
/// suite rather than a detail of one of them.
const PLATFORMS: [Platform; 4] =
    [Platform::MacOs, Platform::Linux, Platform::Windows, Platform::Unknown];

/// Every power state worth distinguishing, including the unreadable one.
fn power_readings() -> Vec<Reading<Power>> {
    vec![
        Reading::known(Power::on_mains(100)),
        Reading::known(Power::mains_only()),
        Reading::known(Power::on_battery(95)),
        Reading::known(Power::on_battery(40)),
        Reading::known(Power::on_battery(3)),
        Reading::unavailable("no battery interface"),
    ]
}

fn disk_readings() -> Vec<Reading<Disk>> {
    vec![
        Reading::known(Disk::new("/", 400 * GIB, 512 * GIB)),
        Reading::known(Disk::new("/", 5 * GIB, 512 * GIB)),
        Reading::known(Disk::new("/", GIB / 2, 512 * GIB)),
        Reading::unavailable("`df` did not answer"),
    ]
}

fn clock_readings() -> Vec<Reading<Clock>> {
    vec![
        Reading::known(Clock::in_zone(540, "Asia/Tokyo")),
        Reading::known(Clock::at_offset(-420)),
        Reading::unavailable("no time zone interface"),
    ]
}

fn skew_readings() -> Vec<Reading<Skew>> {
    vec![
        Reading::known(Skew::new("pool.ntp.org", 2)),
        Reading::known(Skew::new("pool.ntp.org", 90)),
        Reading::known(Skew::new("pool.ntp.org", -4000)),
        Reading::unavailable("no reference clock reachable"),
    ]
}

fn font_readings() -> Vec<Reading<InstalledFonts>> {
    vec![
        Reading::known(["Inter", "IBM Plex Mono"].into_iter().collect()),
        Reading::known(InstalledFonts::default()),
        Reading::unavailable("fonts could not be listed"),
    ]
}

fn process_readings() -> Vec<Reading<RunningProcesses>> {
    vec![
        Reading::known(["Finder", "code"].into_iter().collect()),
        Reading::known(["zoom.us", "obs"].into_iter().collect()),
        Reading::unavailable("`ps` did not answer"),
    ]
}

fn network_readings() -> Vec<Reading<Network>> {
    vec![
        Reading::known(Network::reachable("1.1.1.1:443", 18)),
        Reading::known(Network::captive("1.1.1.1:443", 40)),
        Reading::known(Network::offline("1.1.1.1:443")),
        Reading::unavailable("no socket"),
    ]
}

/// Both display facts, including the arrangement no platform would name.
fn display_readings() -> Vec<Reading<Displays>> {
    let laptop = || Display::new(3024, 1964).drawn_at(1512, 982).named("Color LCD").primary();

    vec![
        Reading::known(Displays::new([laptop()]).extended()),
        Reading::known(Displays::new([laptop(), Display::new(1920, 1080)]).extended()),
        Reading::known(Displays::new([laptop(), Display::new(1024, 768)]).mirrored()),
        // A platform that enumerated its screens and refused to say how they
        // are arranged, which is Windows every time.
        Reading::known(Displays::new([laptop()])),
        Reading::known(Displays::new([])),
        Reading::unavailable("`xrandr` could not be run"),
    ]
}

fn notification_readings() -> Vec<Reading<Notifications>> {
    vec![
        Reading::known(Notifications::Silenced),
        Reading::known(Notifications::Allowed),
        Reading::unavailable("this macOS keeps no Focus state where slidx looks"),
    ]
}

fn audio_readings() -> Vec<Reading<Audio>> {
    vec![
        Reading::known(Audio::playing_at(60)),
        Reading::known(Audio::playing_at(4)),
        Reading::known(Audio::muted_at(70)),
        Reading::known(Audio::level_only(35)),
        Reading::unavailable("Windows exposes no output level a command line can read"),
    ]
}

fn expectations() -> Vec<Expectation> {
    vec![
        Expectation::default(),
        Expectation::default()
            .at_venue_offset(540)
            .with_venue_zone("Asia/Tokyo")
            .with_font_stack("sans", "Inter, sans-serif")
            .with_font_stack("mono", "'IBM Plex Mono', monospace"),
        // A theme that names a font nobody has and no generic to fall back to:
        // the one shape the fonts check is allowed to fail on.
        Expectation::default().at_venue_offset(0).with_font_stack("display", "Söhne"),
    ]
}

/// Every combination of the readings above — several thousand machines that do
/// not exist, which is the point.
fn every_environment() -> Vec<Environment> {
    let mut environments = Vec::new();

    for power in power_readings() {
        for disk in disk_readings() {
            for clock in clock_readings() {
                for skew in skew_readings() {
                    for fonts in font_readings() {
                        for processes in process_readings() {
                            for network in network_readings() {
                                for expected in expectations() {
                                    environments.push(
                                        Environment::new()
                                            .with_power(power.clone())
                                            .with_disk(disk.clone())
                                            .with_clock(clock.clone())
                                            .with_skew(skew.clone())
                                            .with_fonts(fonts.clone())
                                            .with_processes(processes.clone())
                                            .with_network(network.clone())
                                            .expecting(expected),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    environments
}

/// Every state of the three platform readings, on every platform, against a
/// machine that is otherwise ready.
///
/// A second cross product rather than four more loops above, and the reason is
/// arithmetic: folded in, the list would be over a million machines and the
/// suite would stop being something anybody runs. The claims below are about
/// the union of the two, which is what they were always about.
fn every_platform_environment() -> Vec<Environment> {
    let mut environments = Vec::new();

    for displays in display_readings() {
        for notifications in notification_readings() {
            for audio in audio_readings() {
                for platform in PLATFORMS {
                    environments.push(
                        healthy()
                            .with_displays(displays.clone())
                            .with_notifications(notifications.clone())
                            .with_audio(audio.clone())
                            .on(platform),
                    );
                }
            }
        }
    }

    environments
}

/// Every machine the suite makes claims over.
fn all_environments() -> Vec<Environment> {
    let mut all = every_environment();
    all.extend(every_platform_environment());

    all
}

/// A machine that is genuinely ready, with everything the deck expects declared.
fn healthy() -> Environment {
    Environment::new()
        .with_power(Reading::known(Power::on_mains(100)))
        .with_disk(Reading::known(Disk::new("/", 400 * GIB, 512 * GIB)))
        .with_clock(Reading::known(Clock::in_zone(540, "Asia/Tokyo")))
        .with_skew(Reading::known(Skew::new("pool.ntp.org", 2)))
        .with_fonts(Reading::known(["Inter", "IBM Plex Mono"].into_iter().collect()))
        .with_processes(Reading::known(["Finder", "code"].into_iter().collect()))
        .with_network(Reading::known(Network::reachable("1.1.1.1:443", 18)))
        .with_displays(Reading::known(
            Displays::new([
                Display::new(3024, 1964).drawn_at(1512, 982).named("Color LCD").primary(),
                Display::new(1920, 1080).named("EPSON"),
            ])
            .extended(),
        ))
        .with_notifications(Reading::known(Notifications::Silenced))
        .with_audio(Reading::known(Audio::playing_at(60)))
        .on(Platform::MacOs)
        .expecting(
            Expectation::default()
                .at_venue_offset(540)
                .with_venue_zone("Asia/Tokyo")
                .with_font_stack("sans", "Inter, sans-serif")
                .with_font_stack("mono", "'IBM Plex Mono', monospace"),
        )
}

#[test]
fn no_machine_produces_a_finding_the_speaker_cannot_act_on() {
    // The rule the crate lives by. A speaker with ninety seconds and a red line
    // that says only "disk space low" has been given nothing.
    for environment in all_environments() {
        for finding in slidx_doctor::run(&environment).attention() {
            assert!(
                !finding.is_noise(),
                "{} reported {:?} with no remedy: {}",
                finding.check,
                finding.status,
                finding.detail
            );
        }
    }
}

#[test]
fn no_finding_is_ever_left_without_something_to_say() {
    // A blank detail is a line on the report that means nothing at all.
    for environment in all_environments() {
        for finding in slidx_doctor::run(&environment).iter() {
            assert!(!finding.detail.trim().is_empty(), "{} said nothing", finding.check);
        }
    }
}

#[test]
fn every_report_carries_exactly_one_finding_per_registered_check() {
    // Fixed length, whatever the machine. A report that gets shorter when
    // things are fine cannot be told apart from one that skipped a check.
    for environment in all_environments() {
        let report = slidx_doctor::run(&environment);

        assert_eq!(report.len(), check::ALL.len());
        for check in check::ALL {
            assert!(report.get(check.id).is_some(), "{} is missing from the report", check.id);
        }
    }
}

#[test]
fn a_machine_with_nothing_measured_reports_no_passes_it_did_not_earn() {
    // The failure this crate exists to avoid: a green report about a machine
    // nobody could read. Only the fonts check may pass, and only because a deck
    // that names no fonts has nothing that can break.
    let report = slidx_doctor::run(&Environment::new());

    for finding in report.iter() {
        assert!(
            finding.status != Status::Pass || finding.check == "fonts",
            "{} passed on a machine nobody measured: {}",
            finding.check,
            finding.detail
        );
    }
}

#[test]
fn an_unavailable_reading_reports_unknown_rather_than_pass_for_every_check() {
    // Checked one reading at a time against an otherwise healthy machine, so a
    // check cannot pass by borrowing another reading's good news.
    let cases: Vec<(&str, Environment)> = vec![
        ("power", healthy().with_power(Reading::unavailable("no battery interface"))),
        ("disk", healthy().with_disk(Reading::unavailable("`df` did not answer"))),
        ("clock/zone", healthy().with_clock(Reading::unavailable("no time zone interface"))),
        ("clock/skew", healthy().with_skew(Reading::unavailable("no reference clock"))),
        ("screen-capture", healthy().with_processes(Reading::unavailable("`ps` did not answer"))),
        ("network", healthy().with_network(Reading::unavailable("no socket"))),
        ("display/mirroring", healthy().with_displays(Reading::unavailable("no xrandr"))),
        ("display/resolution", healthy().with_displays(Reading::unavailable("no xrandr"))),
        ("notifications", healthy().with_notifications(Reading::unavailable("no Focus state"))),
        ("audio", healthy().with_audio(Reading::unavailable("no output level here"))),
    ];

    for (id, environment) in cases {
        let report = slidx_doctor::run(&environment);
        let finding = report.get(id).expect("every check reports");

        assert_eq!(finding.status, Status::Unknown, "{id} did not report an unknown");
        assert!(finding.remedy.is_some(), "{id} left an unknown without a remedy");
    }
}

#[test]
fn a_machine_that_is_genuinely_ready_reports_nothing_at_all() {
    // The other half of the same promise: a doctor that always finds something
    // is a doctor nobody runs twice.
    let report = slidx_doctor::run(&healthy());

    assert!(
        report.is_healthy(),
        "healthy machine reported: {:?}",
        report.attention().collect::<Vec<_>>()
    );
    assert_eq!(report.status(), Status::Pass);
    assert_eq!(report.attention().count(), 0);
}

#[test]
fn a_ready_machine_at_a_venue_with_no_network_has_nothing_red() {
    // The normal venue. The deck works offline by design, so an offline venue
    // must not produce a single failure — only the two informational lines.
    let environment = healthy()
        .with_network(Reading::known(Network::offline("1.1.1.1:443")))
        .with_skew(Reading::unavailable("no reference clock reachable"));

    let report = slidx_doctor::run(&environment);

    assert_eq!(report.tally(Status::Fail), 0, "an offline venue produced a failure");
    assert_eq!(report.get("network").map(|finding| finding.status), Some(Status::Warn));
}

#[test]
fn the_network_check_never_fails_on_any_machine() {
    // A deck renders offline. If this check could ever go red it would be
    // training speakers to walk on stage having dismissed a red line.
    for environment in all_environments() {
        let report = slidx_doctor::run(&environment);
        let network = report.get("network").expect("the network check always reports");

        assert_ne!(network.status, Status::Fail, "network failed: {}", network.detail);
    }
}

#[test]
fn the_screen_capture_check_never_fails_on_any_machine() {
    // A hybrid talk needs the conferencing app open. Informational means
    // informational.
    for environment in all_environments() {
        let report = slidx_doctor::run(&environment);
        let capture = report.get("screen-capture").expect("the capture check always reports");

        assert_ne!(capture.status, Status::Fail, "screen-capture failed: {}", capture.detail);
    }
}

#[test]
fn mirroring_is_the_only_one_of_the_three_platform_settings_that_can_go_red() {
    // Deliberate, and worth a test rather than a comment. Mirroring removes
    // presenter view outright. A notification is an embarrassment and a muted
    // laptop was probably muted on purpose, so a doctor that went red over
    // either would be arguing with a speaker who was being careful — and red
    // lines that get argued with stop being read.
    for environment in all_environments() {
        let report = slidx_doctor::run(&environment);

        for id in ["notifications", "audio", "display/resolution"] {
            let finding = report.get(id).expect("every check reports");
            assert_ne!(finding.status, Status::Fail, "{id} failed: {}", finding.detail);
        }
    }
}

#[test]
fn no_platform_is_ever_sent_to_another_platforms_menu() {
    // The reason the platform is carried in the environment at all. A speaker
    // looking for a macOS pane on a Windows laptop has been sent away at the
    // one moment they cannot afford it, so every remedy that names a setting
    // has to name a different one per platform.
    let broken = healthy()
        .with_displays(Reading::known(
            Displays::new([Display::new(1920, 1080), Display::new(1920, 1080)]).mirrored(),
        ))
        .with_notifications(Reading::known(Notifications::Allowed));

    for id in ["display/mirroring", "notifications"] {
        let mut remedies: Vec<String> = PLATFORMS
            .iter()
            .map(|platform| {
                slidx_doctor::run(&broken.clone().on(*platform))
                    .get(id)
                    .and_then(|finding| finding.remedy.clone())
                    .unwrap_or_default()
            })
            .collect();

        let named = remedies.len();
        remedies.sort();
        remedies.dedup();

        assert_eq!(remedies.len(), named, "{id} gives two platforms the same menu");
    }
}

#[test]
fn a_reading_a_platform_will_not_give_is_unknown_rather_than_absent_from_the_report() {
    // Windows reports no output level and will not say whether Focus assist is
    // on. Both lines still appear, still ask for something, and still say why —
    // a line that vanished would be indistinguishable from one that passed.
    let windows = healthy()
        .on(Platform::Windows)
        .with_audio(Reading::unavailable("Windows exposes no output level a command line can read"))
        .with_notifications(Reading::unavailable(
            "Windows does not report whether Focus assist is on",
        ));

    let report = slidx_doctor::run(&windows);

    for id in ["audio", "notifications"] {
        let finding = report.get(id).expect("every check reports, on every platform");

        assert_eq!(finding.status, Status::Unknown, "{id} claimed something nobody read");
        assert!(finding.detail.contains("Windows"), "{id} did not say why: {}", finding.detail);
        assert!(finding.remedy.is_some(), "{id} left an unknown with nothing to do");
    }
}

#[test]
fn every_report_is_ordered_worst_first() {
    // Under time pressure only the top of the list gets read, so what has to be
    // fixed has to be there.
    for environment in all_environments() {
        let report = slidx_doctor::run(&environment);
        let urgencies: Vec<u8> = report.iter().map(|finding| finding.status.urgency()).collect();

        assert!(urgencies.windows(2).all(|pair| pair[0] <= pair[1]), "out of order: {urgencies:?}");
    }
}

#[test]
fn findings_of_equal_severity_always_appear_in_registry_order() {
    // The same machine must print the same report every time. Rows that move
    // between runs have to be re-read from the top.
    for environment in all_environments() {
        let report = slidx_doctor::run(&environment);

        let positions: Vec<(u8, usize)> = report
            .iter()
            .map(|finding| (finding.status.urgency(), check::order_of(finding.check)))
            .collect();

        assert!(
            positions.windows(2).all(|pair| pair[0] <= pair[1]),
            "unstable ordering: {positions:?}"
        );
    }
}

#[test]
fn the_same_readings_always_produce_the_same_report() {
    // Checks are pure, which is what makes a bug report reproducible from a
    // captured environment rather than from a description of a room.
    let environment = healthy().with_power(Reading::known(Power::on_battery(30)));

    assert_eq!(slidx_doctor::run(&environment), slidx_doctor::run(&environment));
}

#[test]
fn reading_a_real_machine_and_running_the_suite_produces_a_full_report() {
    // The one end-to-end path: probe an actual machine, run every check. It
    // cannot assert what this machine's battery says — that is the whole reason
    // the checks take injected readings — only that the two halves fit together
    // and that nothing hangs or panics on the way.
    let environment = probe::read(&probe::Request::offline());
    let report = slidx_doctor::run(&environment);

    assert_eq!(report.len(), check::ALL.len());
    assert!(report.attention().all(|finding| finding.remedy.is_some()));
}
