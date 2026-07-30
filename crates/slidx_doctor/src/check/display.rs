//! The screens, and whether presenter view has anywhere to go.
//!
//! Two checks over one reading, for the same reason the clock is two: they fail
//! apart. A machine will report its resolutions and refuse to say whether the
//! arrangement is mirrored, and one verdict speaking for both would have to
//! choose between a reading that was taken and one that was not.
//!
//! Mirroring is a **failure** rather than a warning, and it is the only new
//! check here that can go red. A mirrored pair is one screen wearing two
//! cables, so the notes, the clock and the next slide have nowhere to be — and
//! when it is the speaker's own setting rather than the venue's, the fix is two
//! clicks. That is exactly what `Fail` means here: fix this before you start.
//!
//! Resolution almost never fires, on purpose. What it is for is the line it
//! prints when it passes: a speaker who can see that the projector is attached
//! and at what size knows something no other check tells them.

use crate::environment::{Displays, Environment, Platform};
use crate::finding::Finding;

const MIRRORING: &str = "display/mirroring";
const RESOLUTION: &str = "display/resolution";

/// The slide `slidx lint` measures a deck against — see
/// `slidx_lint::RenderTarget`. Every image rule and every type rule in the
/// linter assumed these pixels, so it is the number the resolution check
/// compares to rather than one invented here.
const REFERENCE: &str = "1920x1080";

/// Two thirds of that reference, below which a deck is being resampled rather
/// than merely scaled: thin type loses its stems and a screenshot that was
/// sharp in the editor is not.
const MINIMUM_WIDTH: u32 = 1280;
const MINIMUM_HEIGHT: u32 = 720;

pub fn mirroring(environment: &Environment) -> Finding {
    let Some(displays) = attached(environment) else {
        return unreadable(environment, MIRRORING);
    };

    match displays.is_mirrored() {
        Some(true) => Finding::fail(
            MIRRORING,
            format!("{} displays, mirrored", displays.len()),
            format!(
                "turn mirroring off in {} — presenter view needs a second screen, and mirrored \
                 there is only one. If the venue's switcher forces it, plan on the deck alone: \
                 your notes and the clock will not be anywhere you can see them",
                display_settings(environment.platform)
            ),
        ),
        Some(false) if displays.len() > 1 => Finding::pass(
            MIRRORING,
            format!("{} displays, extended — presenter view has a screen", displays.len()),
        ),
        // One screen mirrors nothing. A speaker running this at their desk
        // before the projector exists is the normal case, and a warning there
        // is a line they learn to skip.
        Some(false) => Finding::pass(MIRRORING, "one display, so nothing is being mirrored"),
        None => Finding::unknown(
            MIRRORING,
            format!(
                "{} does not say whether these displays are mirrored",
                environment.platform.as_name()
            ),
            format!(
                "open {} and check the arrangement says extend rather than duplicate — \
                 presenter view has nowhere to open on a mirrored pair",
                display_settings(environment.platform)
            ),
        ),
    }
}

pub fn resolution(environment: &Environment) -> Finding {
    let Some(displays) = attached(environment) else {
        return unreadable(environment, RESOLUTION);
    };

    let detail = format!("{}: {}", counted(displays), displays.labels().join(", "));

    let smallest = displays.smallest();
    let too_small = smallest.is_some_and(|screen| {
        let size = screen.drawn_size();
        size.width < MINIMUM_WIDTH || size.height < MINIMUM_HEIGHT
    });

    if too_small {
        return Finding::warn(
            RESOLUTION,
            detail,
            format!(
                "slidx lays a deck out against a {REFERENCE} slide and `slidx lint` measured \
                 your type and your images against it. A screen this small is resampling both, \
                 so check the projector is running at its own native resolution rather than a \
                 mode the laptop picked"
            ),
        );
    }

    Finding::pass(RESOLUTION, detail)
}

/// The reading, when there is one and it holds at least one screen.
///
/// A machine with no screen at all is not a machine with a fine display setup:
/// it is a build agent, or a laptop reached over SSH, and either way nobody is
/// looking at the thing this check is about.
fn attached(environment: &Environment) -> Option<&Displays> {
    environment.displays.value().filter(|displays| !displays.is_empty())
}

/// The `Unknown` both checks report, whether the reading failed or came back
/// with no screens in it.
fn unreadable(environment: &Environment, check: &'static str) -> Finding {
    let detail = match environment.displays.value() {
        Some(_) => "no display is attached to this machine".to_string(),
        None => format!(
            "the displays could not be read: {}",
            environment.displays.reason().unwrap_or("no reason given")
        ),
    };

    Finding::unknown(
        check,
        detail,
        format!(
            "run this on the machine you will speak from with the projector already plugged in, \
             and check the arrangement in {}",
            display_settings(environment.platform)
        ),
    )
}

/// "one display" or "3 displays", so the sentence reads.
fn counted(displays: &Displays) -> String {
    match displays.len() {
        1 => "one display".to_string(),
        count => format!("{count} displays"),
    }
}

/// Where this platform keeps the display arrangement.
///
/// Named rather than described, because a remedy a speaker has to go hunting
/// through a settings app for is a remedy they abandon.
fn display_settings(platform: Platform) -> &'static str {
    match platform {
        Platform::MacOs => "System Settings > Displays",
        Platform::Windows => "Win+P, or Settings > System > Display",
        Platform::Linux => "your desktop's display settings",
        Platform::Unknown => "your display settings",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{Display, Reading};
    use crate::finding::Status;

    fn seeing(displays: Displays) -> Environment {
        Environment::new().with_displays(Reading::known(displays))
    }

    fn laptop() -> Display {
        Display::new(3024, 1964).drawn_at(1512, 982).named("Color LCD").primary()
    }

    fn projector() -> Display {
        Display::new(1920, 1080).named("EPSON")
    }

    #[test]
    fn a_mirrored_arrangement_fails_because_presenter_view_has_nowhere_to_open() {
        // The stage failure this check exists for, and the one the README lists
        // by hand. It is red rather than amber because for the speaker whose
        // own setting it is, the fix is two clicks.
        let finding = mirroring(&seeing(Displays::new([laptop(), projector()]).mirrored()));

        assert_eq!(finding.status, Status::Fail);
        assert!(finding.detail.contains("mirrored"), "got: {}", finding.detail);
    }

    #[test]
    fn the_mirroring_remedy_names_the_menu_for_the_platform_it_was_read_on() {
        // The reason the platform is in the environment at all. "Turn mirroring
        // off" without a menu is thirty seconds of hunting, on the one machine
        // where thirty seconds is not available.
        let mirrored = Displays::new([laptop(), projector()]).mirrored();

        let named = |platform| {
            mirroring(&seeing(mirrored.clone()).on(platform)).remedy.unwrap_or_default()
        };

        assert!(named(Platform::MacOs).contains("System Settings"), "{}", named(Platform::MacOs));
        assert!(named(Platform::Windows).contains("Win+P"), "{}", named(Platform::Windows));
        assert!(named(Platform::Linux).contains("desktop's"), "{}", named(Platform::Linux));
    }

    #[test]
    fn a_platform_nobody_named_gets_a_remedy_that_names_no_menu() {
        // Naming the wrong settings app is worse than naming none: a speaker
        // looking for a macOS pane on a Linux laptop has been sent away.
        let finding = mirroring(&seeing(Displays::new([laptop(), projector()]).mirrored()));
        let remedy = finding.remedy.unwrap_or_default();

        assert!(remedy.contains("your display settings"), "got: {remedy}");
        assert!(!remedy.contains("System Settings"), "got: {remedy}");
    }

    #[test]
    fn two_extended_displays_pass_because_presenter_view_has_a_screen() {
        let finding = mirroring(&seeing(Displays::new([laptop(), projector()]).extended()));

        assert_eq!(finding.status, Status::Pass);
        assert!(finding.detail.contains("extended"), "got: {}", finding.detail);
    }

    #[test]
    fn one_display_passes_rather_than_warning_that_the_projector_is_missing() {
        // A speaker runs this at their desk before the projector exists. A
        // check that fires every time is one they learn to skip, and it costs
        // the lines that matter.
        let finding = mirroring(&seeing(Displays::new([laptop()]).extended()));

        assert_eq!(finding.status, Status::Pass);
    }

    #[test]
    fn a_platform_that_will_not_say_reports_unknown_rather_than_extended() {
        // Windows in duplicate mode shows one logical screen, so it cannot tell
        // duplicating from single-monitor. Reporting extended there would send
        // a speaker on stage expecting presenter view to open.
        let finding = mirroring(&seeing(Displays::new([laptop()])).on(Platform::Windows));

        assert_eq!(finding.status, Status::Unknown);
        assert!(finding.remedy.is_some());
    }

    #[test]
    fn an_unreadable_display_list_is_unknown_for_both_checks_and_never_a_pass() {
        let environment =
            Environment::new().with_displays(Reading::unavailable("xrandr is not installed"));

        for finding in [mirroring(&environment), resolution(&environment)] {
            assert_eq!(finding.status, Status::Unknown);
            assert!(finding.detail.contains("xrandr"), "got: {}", finding.detail);
            assert!(finding.remedy.is_some());
        }
    }

    #[test]
    fn a_machine_with_no_screen_at_all_is_unknown_rather_than_a_clean_bill() {
        // A build agent, or a laptop reached over SSH. Nobody is looking at the
        // thing these checks are about, so neither may report green.
        let environment = seeing(Displays::new([]).extended());

        for finding in [mirroring(&environment), resolution(&environment)] {
            assert_eq!(finding.status, Status::Unknown);
            assert!(finding.detail.contains("no display"), "got: {}", finding.detail);
        }
    }

    #[test]
    fn the_resolution_line_names_every_screen_and_both_of_a_scaled_panels_sizes() {
        // The value of this check is the line it prints when it passes: which
        // screens are attached, and how big each one is.
        let finding = resolution(&seeing(Displays::new([laptop(), projector()]).extended()));

        assert_eq!(finding.status, Status::Pass);
        assert!(finding.detail.contains("2 displays"), "got: {}", finding.detail);
        assert!(finding.detail.contains("Color LCD 1512x982 (3024x1964 pixels)"));
        assert!(finding.detail.contains("EPSON 1920x1080"));
    }

    #[test]
    fn a_projector_below_two_thirds_of_the_slide_slidx_lays_out_against_warns() {
        // 1024x768 is a real venue projector. Everything `slidx lint` measured
        // assumed a 1920x1080 slide, and at this size both the type and the
        // images are being resampled rather than scaled.
        let finding =
            resolution(&seeing(Displays::new([laptop(), Display::new(1024, 768)]).extended()));

        assert_eq!(finding.status, Status::Warn);
        assert!(finding.remedy.unwrap_or_default().contains(REFERENCE));
    }

    #[test]
    fn a_small_screen_is_judged_on_the_points_it_draws_and_not_its_pixels() {
        // A panel with 2560 pixels drawn at 1280 points has 1280 points of room
        // for the deck. Reasoning about the pixels would pass a screen that is
        // resampling everything on it.
        let squeezed = Display::new(2560, 1440).drawn_at(1280, 720);
        let cramped = Display::new(2560, 1440).drawn_at(1024, 576);

        assert_eq!(resolution(&seeing(Displays::new([squeezed]).extended())).status, Status::Pass);
        assert_eq!(resolution(&seeing(Displays::new([cramped]).extended())).status, Status::Warn);
    }

    #[test]
    fn the_smallest_screen_decides_the_verdict_rather_than_the_laptop_panel() {
        // The laptop is never the screen that loses the room. Averaging, or
        // taking the primary, would hide the projector that does.
        let finding =
            resolution(&seeing(Displays::new([laptop(), Display::new(800, 600)]).extended()));

        assert_eq!(finding.status, Status::Warn);
    }

    #[test]
    fn one_display_is_counted_in_words_so_the_line_reads_as_a_sentence() {
        let finding = resolution(&seeing(Displays::new([projector()]).extended()));

        assert!(finding.detail.starts_with("one display:"), "got: {}", finding.detail);
    }

    #[test]
    fn neither_check_ever_reports_a_finding_without_something_to_say() {
        // Both are reached with an empty list, an unavailable reading and every
        // arrangement, so a blank detail is a real reachable state.
        let environments = [
            Environment::new(),
            seeing(Displays::new([])),
            seeing(Displays::new([laptop()])),
            seeing(Displays::new([laptop(), projector()]).mirrored()),
            seeing(Displays::new([laptop(), projector()]).extended()),
        ];

        for environment in environments {
            for finding in [mirroring(&environment), resolution(&environment)] {
                assert!(!finding.detail.trim().is_empty());
                assert!(!finding.is_noise(), "{} left {:?} unactionable", finding.check, finding);
            }
        }
    }
}
