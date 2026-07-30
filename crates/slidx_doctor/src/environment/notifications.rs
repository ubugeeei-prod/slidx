//! Whether the machine will interrupt.
//!
//! Two states and no third, because the third one lives in
//! [`Reading`](crate::Reading) where every other unmeasured thing lives. A
//! `Notifications` value means somebody read the setting; not being able to
//! read it is an unavailable reading, and the difference is the whole point of
//! this line existing at all.
//!
//! There is no percentage here and nothing to tune. A banner either lands on
//! the slide in front of two hundred people or it does not.

use serde::{Deserialize, Serialize};

/// What the machine will do with a notification during the talk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Notifications {
    /// Banners are suppressed — Do Not Disturb, a Focus mode, Focus assist, or
    /// notifications switched off outright. Which of those it is does not
    /// change what the audience sees, so the reading does not distinguish them.
    Silenced,
    /// A message will appear on the screen the room is looking at.
    Allowed,
}

impl Notifications {
    /// True when nothing will land on the slide.
    pub fn is_silenced(self) -> bool {
        self == Self::Silenced
    }

    /// Stable lowercase name, for JSON and for a message.
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Silenced => "silenced",
            Self::Allowed => "allowed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::Reading;

    #[test]
    fn a_machine_that_will_interrupt_is_a_different_answer_from_one_nobody_read() {
        // The distinction this whole crate is built on, at the one reading
        // where a platform is most likely to refuse to answer.
        let read: Reading<Notifications> = Reading::known(Notifications::Allowed);
        let unread: Reading<Notifications> = Reading::unavailable("no Focus state slidx can read");

        assert_eq!(read.value(), Some(&Notifications::Allowed));
        assert_eq!(unread.value(), None);
    }

    #[test]
    fn only_the_silenced_state_means_nothing_lands_on_the_slide() {
        assert!(Notifications::Silenced.is_silenced());
        assert!(!Notifications::Allowed.is_silenced());
    }

    #[test]
    fn the_two_states_serialise_to_the_tokens_they_print() {
        for state in [Notifications::Silenced, Notifications::Allowed] {
            let json = serde_json::to_string(&state).unwrap_or_default();
            assert_eq!(json, format!("\"{}\"", state.as_token()));
        }
    }
}
