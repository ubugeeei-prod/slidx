//! What comes out of the speakers.
//!
//! Both halves are optional inside a reading that was taken, and each one is a
//! real platform answer rather than a hedge. An output with no software volume
//! — HDMI to a projector, an external interface — genuinely has no level for
//! the machine to report, because the knob is on the hardware. A tool that
//! reads mute and not level, or level and not mute, is the normal case on more
//! than one desktop.
//!
//! The output *device* is deliberately not here. No platform hands over which
//! device is selected in the same call that gives the level, and a pre-flight
//! that spends a second subprocess per reading is a pre-flight nobody waits
//! for — so the audio check's remedy tells the speaker to look at where the
//! sound is going, which is the failure a device name would have caught.

use serde::{Deserialize, Serialize};

/// The output level, and whether anything is coming out at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Audio {
    /// Output level, 0-100. `None` on an output whose volume lives on the
    /// hardware, where the machine truly does not know.
    pub level_percent: Option<u8>,
    /// `None` where the platform's tool reports a level and no mute state.
    pub muted: Option<bool>,
}

impl Audio {
    /// Both halves read, and nothing is muted.
    pub fn playing_at(level_percent: u8) -> Self {
        Self { level_percent: Some(level_percent.min(100)), muted: Some(false) }
    }

    /// Both halves read, and the output is muted. The level survives, because
    /// "muted at 70%" and "muted at 0%" are different amounts of work to undo.
    pub fn muted_at(level_percent: u8) -> Self {
        Self { level_percent: Some(level_percent.min(100)), muted: Some(true) }
    }

    /// A tool that gave a level and said nothing about mute.
    pub fn level_only(level_percent: u8) -> Self {
        Self { level_percent: Some(level_percent.min(100)), muted: None }
    }

    /// Nothing will be heard: muted, or turned all the way down.
    pub fn is_silent(&self) -> bool {
        self.muted == Some(true) || self.level_percent == Some(0)
    }

    /// True when the reading came back holding neither half, which a check has
    /// to treat as unmeasured rather than as fine.
    pub fn says_nothing(&self) -> bool {
        self.level_percent.is_none() && self.muted.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_muted_output_keeps_the_level_it_was_muted_at() {
        // Muted at 70% is one keystroke from working. Muted at 0% is two, and
        // a speaker who unmutes and hears nothing has spent their thirty
        // seconds on the wrong half.
        let audio = Audio::muted_at(70);

        assert_eq!(audio.level_percent, Some(70));
        assert_eq!(audio.muted, Some(true));
    }

    #[test]
    fn nothing_comes_out_of_a_muted_output_or_one_turned_all_the_way_down() {
        assert!(Audio::muted_at(70).is_silent());
        assert!(Audio::playing_at(0).is_silent());
        assert!(!Audio::playing_at(1).is_silent());
    }

    #[test]
    fn a_level_with_no_mute_state_is_not_assumed_to_be_unmuted() {
        // Some tools report one and not the other. Reading the absence as
        // "not muted" would turn a silent demo into a green line.
        let audio = Audio::level_only(60);

        assert_eq!(audio.muted, None);
        assert!(!audio.is_silent());
        assert!(!audio.says_nothing());
    }

    #[test]
    fn a_reading_holding_neither_half_admits_it_says_nothing() {
        // Reachable from a tool that answered without any number in it. The
        // check has to report that as unmeasured rather than as fine.
        let audio = Audio { level_percent: None, muted: None };

        assert!(audio.says_nothing());
        assert!(!audio.is_silent());
    }

    #[test]
    fn a_level_over_a_hundred_percent_is_clamped() {
        // Both PipeWire and PulseAudio will happily report a boosted sink
        // above 100%, and a percentage that cannot happen makes the line look
        // broken at the moment it needs to be believed.
        assert_eq!(Audio::playing_at(153).level_percent, Some(100));
        assert_eq!(Audio::muted_at(200).level_percent, Some(100));
    }
}
