//! Whether anything the deck plays will be heard.
//!
//! A warning and never a failure, for a reason worth writing down: muting a
//! laptop before walking on is a **sensible** thing for a speaker to have done,
//! and a doctor that called it a fault would be arguing with somebody who was
//! being careful. Most decks play nothing. The ones that do fail silently, and
//! nobody in the room says anything for the first thirty seconds, which is why
//! the line is here at all.
//!
//! The threshold is low on purpose. A level that would be quiet through a
//! laptop speaker may be plenty through the room's system, and slidx cannot see
//! which of those the sound is going to — so this warns only where nothing
//! could be heard through anything, and says the rest in the remedy.

use crate::environment::Environment;
use crate::finding::Finding;

const ID: &str = "audio";

/// Below this, no room hears it through anything. Above it, whether the level
/// is enough depends on the amplifier at the other end of a cable slidx cannot
/// see, so guessing would be inventing an opinion about somebody else's venue.
const FAINT_PERCENT: u8 = 20;

pub fn check(environment: &Environment) -> Finding {
    let Some(audio) = environment.audio.value() else {
        return Finding::unknown(
            ID,
            format!(
                "the output level could not be read: {}",
                environment.audio.reason().unwrap_or("no reason given")
            ),
            UNMUTE_AND_CHECK_THE_DEVICE,
        );
    };

    if audio.says_nothing() {
        return Finding::unknown(
            ID,
            "the output answered without a level or a mute state",
            UNMUTE_AND_CHECK_THE_DEVICE,
        );
    }

    if audio.is_silent() {
        return Finding::warn(ID, silent_detail(audio.level_percent), UNMUTE_AND_CHECK_THE_DEVICE);
    }

    match audio.level_percent {
        Some(percent) if percent < FAINT_PERCENT => Finding::warn(
            ID,
            format!("output at {percent}%"),
            "turn it up if anything in the deck plays sound — nothing is audible in a room at \
             this level, through the laptop's own speakers or through the venue's",
        ),
        Some(percent) => Finding::pass(ID, format!("output at {percent}%, not muted")),
        // A tool that reported the mute state and no level. Half a reading, and
        // the half that matters most: nothing is muted.
        None => Finding::pass(ID, "output is not muted, and this platform reports no level"),
    }
}

/// What a muted output says, keeping the level it was muted at.
///
/// Muted at 70% is one keystroke from working and muted at 0% is two, and a
/// speaker who unmutes, hears nothing and gives up has spent their thirty
/// seconds on the wrong half.
fn silent_detail(level_percent: Option<u8>) -> String {
    match level_percent {
        Some(0) => "output is turned all the way down".to_string(),
        Some(percent) => format!("output is muted, at {percent}%"),
        None => "output is muted".to_string(),
    }
}

/// The one remedy every unheard case gets.
///
/// It names the output device even though the reading does not carry one: no
/// platform hands that over cheaply, and a demo playing perfectly into a pair
/// of headphones in somebody's bag is exactly the failure a device name would
/// have caught. Saying so is better than measuring nothing and mentioning
/// nothing.
const UNMUTE_AND_CHECK_THE_DEVICE: &str =
    "unmute and set a level if anything in the deck plays sound, and check where that sound is \
     going — slidx cannot see which output device is selected, and a demo playing into \
     headphones in your bag looks identical from here";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{Audio, Reading};
    use crate::finding::Status;

    fn hearing(audio: Audio) -> Environment {
        Environment::new().with_audio(Reading::known(audio))
    }

    #[test]
    fn a_machine_at_a_usable_level_passes_and_says_what_the_level_is() {
        let finding = check(&hearing(Audio::playing_at(60)));

        assert_eq!(finding.status, Status::Pass);
        assert!(finding.detail.contains("60%"), "got: {}", finding.detail);
    }

    #[test]
    fn a_muted_output_warns_rather_than_fails_because_muting_was_probably_deliberate() {
        // A speaker who muted their laptop on purpose is being careful. Calling
        // that a fault is how a pre-flight starts arguing with its user.
        assert_eq!(check(&hearing(Audio::muted_at(70))).status, Status::Warn);
    }

    #[test]
    fn a_muted_output_says_the_level_it_was_muted_at() {
        // Unmuting a machine that is also at zero and hearing nothing is how a
        // speaker concludes the demo is broken.
        assert!(check(&hearing(Audio::muted_at(70))).detail.contains("70%"));
        assert!(check(&hearing(Audio::playing_at(0))).detail.contains("all the way down"));
    }

    #[test]
    fn a_level_nothing_could_be_heard_at_warns_even_unmuted() {
        assert_eq!(check(&hearing(Audio::playing_at(FAINT_PERCENT - 1))).status, Status::Warn);
        assert_eq!(check(&hearing(Audio::playing_at(FAINT_PERCENT))).status, Status::Pass);
    }

    #[test]
    fn every_unheard_case_is_told_to_check_where_the_sound_is_going() {
        // The failure slidx cannot measure. A demo playing perfectly into
        // headphones in a bag looks identical to a working one from here, so
        // the remedy carries what the reading could not.
        for audio in [Audio::muted_at(70), Audio::playing_at(0)] {
            let remedy = check(&hearing(audio)).remedy.unwrap_or_default();
            assert!(remedy.contains("headphones"), "got: {remedy}");
        }
    }

    #[test]
    fn a_mute_state_with_no_level_is_still_worth_reporting() {
        // HDMI to a projector has no software volume: the knob is on the
        // hardware. The half that was read — nothing is muted — is the half
        // that matters, so it passes rather than reporting an unknown.
        let unmuted_without_a_level = Audio { level_percent: None, muted: Some(false) };
        let finding = check(&hearing(unmuted_without_a_level));

        assert_eq!(finding.status, Status::Pass);
        assert!(finding.detail.contains("no level"), "got: {}", finding.detail);
    }

    #[test]
    fn a_level_with_no_mute_state_is_judged_on_the_level_alone() {
        assert_eq!(check(&hearing(Audio::level_only(55))).status, Status::Pass);
        assert_eq!(check(&hearing(Audio::level_only(3))).status, Status::Warn);
    }

    #[test]
    fn a_reading_that_came_back_holding_neither_half_is_unknown_rather_than_a_pass() {
        // A tool that answered with no number in it has measured nothing, and a
        // green line about nothing is the one failure this crate exists to
        // avoid.
        let empty = Audio { level_percent: None, muted: None };
        let finding = check(&hearing(empty));

        assert_eq!(finding.status, Status::Unknown);
        assert!(finding.remedy.is_some());
    }

    #[test]
    fn an_unreadable_output_is_unknown_and_names_why() {
        // Windows is this case every time: it exposes no volume a command line
        // can read without shipping native code, so slidx says so.
        let environment = Environment::new()
            .with_audio(Reading::unavailable("Windows exposes no output level slidx can read"));
        let finding = check(&environment);

        assert_eq!(finding.status, Status::Unknown);
        assert!(finding.detail.contains("Windows exposes"), "got: {}", finding.detail);
        assert!(finding.remedy.is_some());
    }
}
