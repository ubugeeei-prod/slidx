//! Which operating system a reading was taken from.
//!
//! A value rather than a `#[cfg]` block, and that is load-bearing rather than
//! stylistic. The display arrangement, the Do Not Disturb state and the output
//! volume are each read through one platform's own tool and nothing else's, so
//! behind a `#[cfg]` two of every three branches would be unreachable on any
//! one runner — and a branch no runner reaches is a branch that breaks without
//! anybody finding out until a speaker is standing in a room.
//!
//! As a value it is injected. A test on Linux drives the macOS branch, CI
//! exercises all three on all three, and [`host`](Platform::host) is the single
//! place in the crate where the compile target decides anything.
//!
//! The checks read it too, for a reason that is not about testing: a remedy
//! that says "turn Do Not Disturb on" is useless without saying where that
//! switch lives, and where it lives differs per platform.

use serde::{Deserialize, Serialize};

/// The operating system, as far as the readings are concerned.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    #[serde(rename = "macos")]
    MacOs,
    #[serde(rename = "linux")]
    Linux,
    #[serde(rename = "windows")]
    Windows,
    /// Something else, or nobody said.
    ///
    /// Not "a platform with no settings" — a platform slidx has no way to ask.
    /// A check reading this one names no menu, because naming the wrong menu is
    /// how a speaker spends thirty seconds looking for a switch that is not
    /// there.
    #[default]
    #[serde(rename = "unknown")]
    Unknown,
}

impl Platform {
    /// What this build was compiled for.
    ///
    /// The only place in the new probes where `#[cfg]` decides anything, and
    /// all it decides is which value comes back.
    pub fn host() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::MacOs
        }

        #[cfg(target_os = "linux")]
        {
            Self::Linux
        }

        #[cfg(windows)]
        {
            Self::Windows
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
        {
            Self::Unknown
        }
    }

    /// Stable lowercase name, for JSON and for a message.
    pub fn as_token(self) -> &'static str {
        match self {
            Self::MacOs => "macos",
            Self::Linux => "linux",
            Self::Windows => "windows",
            Self::Unknown => "unknown",
        }
    }

    /// What a speaker calls it, for a remedy that has to name a menu.
    pub fn as_name(self) -> &'static str {
        match self {
            Self::MacOs => "macOS",
            Self::Linux => "Linux",
            Self::Windows => "Windows",
            Self::Unknown => "this platform",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_host_platform_is_one_of_the_three_slidx_ships_for() {
        // Every CI runner is macOS, Linux or Windows, so a build that reports
        // Unknown on one of them means the detection stopped matching the
        // target and every platform reading silently went dark.
        assert!(matches!(Platform::host(), Platform::MacOs | Platform::Linux | Platform::Windows));
    }

    #[test]
    fn a_platform_nobody_named_is_not_mistaken_for_a_real_one() {
        // The default has to be Unknown rather than the host, or an Environment
        // built field by field in a test would claim readings came from a
        // platform nobody said anything about.
        assert_eq!(Platform::default(), Platform::Unknown);
    }

    #[test]
    fn every_platform_has_a_token_and_a_name_that_differ_from_each_others() {
        let platforms = [Platform::MacOs, Platform::Linux, Platform::Windows, Platform::Unknown];

        let mut tokens: Vec<&str> = platforms.iter().map(|p| p.as_token()).collect();
        tokens.sort_unstable();
        tokens.dedup();

        assert_eq!(tokens.len(), platforms.len());
        assert!(platforms.iter().all(|p| !p.as_name().is_empty()));
    }

    #[test]
    fn an_unknown_platform_is_named_without_pretending_to_know_which_one() {
        // A remedy built on this must not say "on Unknown, open System
        // Settings". It has nothing to name, and says so.
        assert_eq!(Platform::Unknown.as_name(), "this platform");
    }

    #[test]
    fn platforms_serialise_to_the_tokens_the_report_prints() {
        for platform in [Platform::MacOs, Platform::Linux, Platform::Windows] {
            let json = serde_json::to_string(&platform).unwrap_or_default();
            assert_eq!(json, format!("\"{}\"", platform.as_token()));
        }
    }
}
