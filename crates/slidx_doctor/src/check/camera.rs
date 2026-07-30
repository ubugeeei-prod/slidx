//! Whether the camera the deck places will actually be there.
//!
//! A camera fails in the last ten minutes or not at all: the laptop that has
//! none, the conferencing app for the same hybrid talk that already has the
//! one there is, the permission that was refused months ago on another site and
//! remembered. Every one of those is discoverable while the speaker is still
//! sitting down, and every one of them is discovered on stage instead.
//!
//! Nothing here is a failure, and that is deliberate. A camera that will not
//! open costs a camera: the tile says so and the deck presents. Marking it red
//! would put a line above the flat battery, which is the one thing on the report
//! that actually ends a talk.
//!
//! **Silent unless the deck asked.** A speaker whose deck places no camera gets
//! one green line whatever their hardware is. A doctor that warned everybody
//! about a webcam they were never going to use is a doctor whose lines stop
//! being read — the same reason Slack is off the screen-capture watch list.
//!
//! **Not the screen-capture check.** That one is about applications that can
//! take the *screen*, and it stays that way: a screen recorder holds no webcam,
//! so quitting OBS is the wrong advice for a camera that will not open. The two
//! read one watch list from two ends rather than keeping two.

use crate::environment::Environment;
use crate::finding::Finding;

const ID: &str = "camera";

pub fn check(environment: &Environment) -> Finding {
    let slides = environment.expected.camera_slides;

    if slides == 0 {
        return Finding::pass(ID, "no slide in this deck places a camera");
    }

    let Some(cameras) = environment.cameras.value() else {
        return Finding::unknown(
            ID,
            format!(
                "{} place a camera and the cameras here could not be listed: {}",
                slide_count(slides),
                environment.cameras.reason().unwrap_or("no reason given")
            ),
            "open anything that shows a camera preview and check you get a picture — the deck \
             presents either way, but the tile will say `no camera found` in front of the room",
        );
    };

    if cameras.is_empty() {
        return Finding::warn(
            ID,
            format!("{} place a camera and this machine has none", slide_count(slides)),
            "plug one in, or take `camera:` off those slides — left as it is, each tile says so \
             on the slide instead of showing a face",
        );
    }

    // Present, and quite possibly already spoken for. This is the failure a
    // speaker cannot see coming: the camera exists, the pre-flight is green, and
    // the browser is refused the device the moment they start.
    let holders = environment
        .processes
        .value()
        .map(|processes| super::processes::conferencing(processes.names()))
        .unwrap_or_default();

    if !holders.is_empty() {
        return Finding::warn(
            ID,
            format!("{} here, and {} running", cameras_found(cameras), holders.join(", ")),
            "a conferencing app holds the camera exclusively on most platforms, so the deck's \
             tile gets nothing. Quit it, or leave the call before you start",
        );
    }

    Finding::pass(ID, format!("{} for {}", cameras_found(cameras), slide_count(slides)))
}

/// "3 slides", or "1 slide". The count is what makes it worth acting on.
fn slide_count(slides: usize) -> String {
    if slides == 1 {
        "1 slide".to_string()
    } else {
        format!("{slides} slides")
    }
}

/// The camera by name when there is one, and a count when there are several.
///
/// A speaker with two knows which is which; a speaker with one wants to see the
/// name, because reading it back is how they tell a working camera from the
/// virtual one a conferencing app installed.
fn cameras_found(cameras: &crate::environment::Cameras) -> String {
    match cameras.names() {
        [only] => only.clone(),
        names => format!("{} cameras", names.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{Cameras, Expectation, Reading, RunningProcesses};
    use crate::finding::Status;

    fn deck_wanting(slides: usize) -> Environment {
        Environment::new().expecting(Expectation::default().wanting_camera_on(slides))
    }

    fn with_cameras(slides: usize, names: &[&str]) -> Environment {
        deck_wanting(slides)
            .with_cameras(Reading::known(names.iter().copied().collect::<Cameras>()))
    }

    fn running(environment: Environment, names: &[&str]) -> Environment {
        environment
            .with_processes(Reading::known(names.iter().copied().collect::<RunningProcesses>()))
    }

    #[test]
    fn a_deck_that_places_no_camera_is_told_so_and_nothing_else() {
        // Every deck anybody has ever written, near enough. A warning here
        // would fire on all of them and teach the speaker to skip the line.
        let environment = Environment::new().with_cameras(Reading::known(Cameras::default()));

        assert_eq!(check(&environment).status, Status::Pass);
    }

    #[test]
    fn a_machine_with_no_camera_is_reported_when_the_deck_wanted_one() {
        // The whole point of a pre-flight: this is knowable at the desk and
        // otherwise discovered from the stage.
        let finding = check(&with_cameras(3, &[]));

        assert_eq!(finding.status, Status::Warn);
        assert!(finding.detail.contains("3 slides"), "got: {}", finding.detail);
        assert!(finding.remedy.is_some());
    }

    #[test]
    fn a_missing_camera_never_fails_because_the_deck_still_presents() {
        // A camera that will not open costs a camera. Marking it red would put
        // it above the flat battery, which is the one thing that ends a talk.
        assert_eq!(check(&with_cameras(1, &[])).status, Status::Warn);
    }

    #[test]
    fn a_camera_that_is_there_is_named_back_to_the_speaker() {
        // Reading the name is how a speaker tells their own camera from the
        // virtual one a conferencing app installed.
        let finding = check(&with_cameras(2, &["FaceTime HD Camera"]));

        assert_eq!(finding.status, Status::Pass);
        assert!(finding.detail.contains("FaceTime HD Camera"), "got: {}", finding.detail);
    }

    #[test]
    fn a_conferencing_app_holding_the_camera_is_the_failure_nobody_sees_coming() {
        // The camera exists, the hardware check is green, and the browser is
        // refused the device the moment the speaker starts.
        let finding = check(&running(with_cameras(1, &["Integrated Camera"]), &["zoom.us"]));

        assert_eq!(finding.status, Status::Warn);
        assert!(finding.detail.contains("Zoom"), "got: {}", finding.detail);
    }

    #[test]
    fn a_screen_recorder_is_not_reported_as_holding_the_camera() {
        // OBS records a screen and holds no webcam. Telling a speaker to quit
        // it would send them after the wrong application.
        let finding = check(&running(with_cameras(1, &["Integrated Camera"]), &["obs"]));

        assert_eq!(finding.status, Status::Pass);
    }

    #[test]
    fn a_conferencing_app_says_nothing_here_when_the_deck_places_no_camera() {
        // The screen-capture check already owns that line, and two warnings
        // about one running application is one warning too many.
        let environment = running(Environment::new(), &["zoom.us"]);

        assert_eq!(check(&environment).status, Status::Pass);
    }

    #[test]
    fn a_camera_list_nobody_could_read_is_unknown_and_never_a_pass() {
        let environment =
            deck_wanting(1).with_cameras(Reading::unavailable("system_profiler is not available"));
        let finding = check(&environment);

        assert_eq!(finding.status, Status::Unknown);
        assert!(finding.detail.contains("system_profiler"));
        assert!(finding.remedy.is_some());
    }

    #[test]
    fn an_unreadable_process_table_does_not_stop_the_camera_being_reported() {
        // Two independent readings. A locked-down machine that will not list
        // its processes can still say whether it has a camera.
        let finding = check(&with_cameras(1, &["Integrated Camera"]));

        assert_eq!(finding.status, Status::Pass);
    }

    #[test]
    fn one_slide_is_counted_in_the_singular() {
        assert!(check(&with_cameras(1, &[])).detail.contains("1 slide "), "reads as plural");
    }
}
