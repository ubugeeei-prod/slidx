//! Whether a message is about to land on the slide.
//!
//! A warning and never a failure. A banner in front of a room is an
//! embarrassment and occasionally a disclosure, but the talk continues — and a
//! doctor that goes red over one is a doctor whose red lines stop being read.
//!
//! This check does not turn Do Not Disturb on, and no flag on `slidx doctor`
//! will. A check that changed a system setting as a side effect of looking at
//! it would leave a speaker's machine altered by a command they ran to *find
//! out* something, which is the one behaviour a diagnostic tool must not have.
//! What it does instead is name the switch, on the platform it was run on,
//! because "turn notifications off" without a menu is thirty seconds of hunting
//! at the moment thirty seconds is not available.

use crate::environment::{Environment, Platform};
use crate::finding::Finding;

const ID: &str = "notifications";

pub fn check(environment: &Environment) -> Finding {
    let Some(notifications) = environment.notifications.value() else {
        return Finding::unknown(
            ID,
            format!(
                "whether notifications are silenced could not be read: {}",
                environment.notifications.reason().unwrap_or("no reason given")
            ),
            format!(
                "silence them by hand before you start: {}",
                where_it_lives(environment.platform)
            ),
        );
    };

    if notifications.is_silenced() {
        return Finding::pass(ID, "notifications are silenced");
    }

    Finding::warn(
        ID,
        "notifications will appear on screen",
        format!(
            "silence them before you start: {}. A banner lands on the slide the room is \
             reading, and the ones that arrive during a talk are the ones you least want \
             projected",
            where_it_lives(environment.platform)
        ),
    )
}

/// Where this platform keeps the switch.
///
/// Named per platform rather than described in general, and `Unknown` names
/// nothing at all — sending a speaker to a menu their machine does not have is
/// worse than sending them to none.
fn where_it_lives(platform: Platform) -> &'static str {
    match platform {
        Platform::MacOs => "Control Centre > Focus > Do Not Disturb",
        Platform::Windows => "Settings > System > Notifications, or the Focus assist button",
        Platform::Linux => "your desktop's notification settings",
        Platform::Unknown => "your platform's notification settings",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{Notifications, Reading};
    use crate::finding::Status;

    fn with(notifications: Notifications) -> Environment {
        Environment::new().with_notifications(Reading::known(notifications))
    }

    #[test]
    fn a_silenced_machine_passes_with_nothing_left_to_do() {
        let finding = check(&with(Notifications::Silenced));

        assert_eq!(finding.status, Status::Pass);
        assert_eq!(finding.remedy, None);
    }

    #[test]
    fn a_machine_that_will_interrupt_warns_rather_than_fails() {
        // A banner is an embarrassment, not the end of the talk. Red here would
        // teach a speaker to walk on stage having dismissed a red line.
        assert_eq!(check(&with(Notifications::Allowed)).status, Status::Warn);
    }

    #[test]
    fn the_remedy_names_the_switch_for_the_platform_it_was_read_on() {
        // Every platform hides this somewhere different, and a remedy that
        // cannot say where is one a speaker abandons.
        let named =
            |platform| check(&with(Notifications::Allowed).on(platform)).remedy.unwrap_or_default();

        assert!(named(Platform::MacOs).contains("Control Centre"), "{}", named(Platform::MacOs));
        assert!(named(Platform::Windows).contains("Focus assist"), "{}", named(Platform::Windows));
        assert!(named(Platform::Linux).contains("desktop's"), "{}", named(Platform::Linux));
    }

    #[test]
    fn a_platform_nobody_named_is_sent_to_no_particular_menu() {
        let remedy = check(&with(Notifications::Allowed)).remedy.unwrap_or_default();

        assert!(remedy.contains("your platform's"), "got: {remedy}");
        assert!(!remedy.contains("Control Centre"), "got: {remedy}");
    }

    #[test]
    fn an_unreadable_focus_state_is_unknown_and_never_a_pass() {
        // The reading this whole line was blocked on for a release. A platform
        // slidx cannot ask has to say so, because a green line here is a
        // speaker who stopped checking.
        let environment = Environment::new()
            .with_notifications(Reading::unavailable("no Focus state slidx can read here"));
        let finding = check(&environment);

        assert_eq!(finding.status, Status::Unknown);
        assert!(finding.detail.contains("no Focus state"), "got: {}", finding.detail);
        assert!(finding.remedy.is_some());
    }

    #[test]
    fn the_unknown_remedy_still_names_the_switch_so_the_speaker_can_do_it_by_hand() {
        // "slidx could not measure this" is only useful next to "here is where
        // you check it in five seconds".
        let environment = Environment::new()
            .with_notifications(Reading::unavailable("nothing to read"))
            .on(Platform::MacOs);

        assert!(check(&environment).remedy.unwrap_or_default().contains("Do Not Disturb"));
    }
}
