//! Everything a check is allowed to know, as data.
//!
//! No check calls the operating system. Every check is a pure function from an
//! [`Environment`] to a [`Finding`](crate::Finding), which is what makes the
//! whole suite testable with no machine state at all — the alternative is a
//! test suite that can only assert what happens to be true of the laptop it
//! runs on, which for a tool about *other people's* laptops is worthless.
//!
//! The struct has two halves, and the split is load-bearing:
//!
//! - **readings** come from the machine and may be missing, so each is a
//!   [`Reading<T>`] that carries the reason it is missing;
//! - **[`Expectation`]** comes from the deck and the booking — the fonts the
//!   theme names, the zone the talk is scheduled in. It is never missing; it is
//!   simply empty when nobody said.
//!
//! A check that compares the two (fonts, time zone) needs both halves present
//! and independent, which is why the expectation does not live inside a
//! reading. [`crate::probe`] is the one module that turns a machine into this.
//!
//! [`Platform`] is in neither half. It is not read off the machine and nobody
//! declares it — it is what this build was compiled for, and it is here because
//! three checks need it to say where a setting lives. "Turn Do Not Disturb on"
//! without naming the menu is thirty seconds of a speaker hunting for a switch.

pub mod audio;
pub mod displays;
pub mod fonts;
pub mod machine;
pub mod notifications;
pub mod platform;

use serde::{Deserialize, Serialize};

pub use audio::Audio;
pub use displays::{Display, Displays, Resolution};
pub use fonts::{FontStack, InstalledFonts};
pub use machine::{Cameras, Clock, Disk, Network, Power, PowerSource, RunningProcesses, Skew};
pub use notifications::Notifications;
pub use platform::Platform;

/// A measurement that may not have been possible.
///
/// The `Unavailable` reason is written for the speaker, not for a log: it ends
/// up inside an [`Status::Unknown`](crate::Status) finding, next to a remedy
/// telling them how to check by hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reading<T> {
    Known(T),
    Unavailable(String),
}

impl<T> Reading<T> {
    pub fn known(value: T) -> Self {
        Self::Known(value)
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self::Unavailable(reason.into())
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Known(value) => Some(value),
            Self::Unavailable(_) => None,
        }
    }

    /// Why the reading is missing, when it is.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Known(_) => None,
            Self::Unavailable(reason) => Some(reason),
        }
    }

    pub fn is_known(&self) -> bool {
        matches!(self, Self::Known(_))
    }
}

impl<T> Default for Reading<T> {
    /// Unavailable, so an `Environment` built field by field in a test starts
    /// out honest: anything the test did not set reads as "not measured"
    /// rather than as a silent pass.
    fn default() -> Self {
        Self::unavailable("not measured")
    }
}

/// What the deck and the booking say the machine should look like.
///
/// Empty by default. An absent expectation is not a pass — a check with
/// nothing to compare against reports `Unknown` and says what to declare.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Expectation {
    /// Font stacks the deck's theme names, in theme order.
    pub fonts: Vec<FontStack>,
    /// UTC offset, in minutes, that the talk is scheduled in.
    ///
    /// Minutes rather than hours because Kathmandu, Adelaide and Chatham
    /// Island all run on offsets that are not whole hours, and a conference in
    /// one of them is exactly the trip where your laptop is on the wrong zone.
    pub venue_offset_minutes: Option<i32>,
    /// Human name for the venue's zone, for the message only.
    pub venue_zone: Option<String>,
    /// How many slides place a speaker camera.
    ///
    /// Zero — the default, and the overwhelmingly common case — means the
    /// camera check has nothing to say, whatever this machine's hardware is.
    /// A doctor that warned every speaker about a webcam they were never going
    /// to use is a doctor whose lines stop being read.
    pub camera_slides: usize,
}

impl Expectation {
    /// Declares the fonts a theme names. Each entry is a CSS font stack, so
    /// `theme.font_sans` can be handed over unparsed.
    pub fn with_font_stack(mut self, role: impl Into<String>, css: &str) -> Self {
        self.fonts.push(FontStack::parse(role, css));
        self
    }

    pub fn at_venue_offset(mut self, minutes: i32) -> Self {
        self.venue_offset_minutes = Some(minutes);
        self
    }

    pub fn with_venue_zone(mut self, zone: impl Into<String>) -> Self {
        self.venue_zone = Some(zone.into());
        self
    }

    /// Declares how many slides of the deck place a speaker camera.
    pub fn wanting_camera_on(mut self, slides: usize) -> Self {
        self.camera_slides = slides;
        self
    }
}

/// The machine, as far as the checks are concerned.
///
/// `Default` reports every reading as unavailable, which is the correct
/// starting point: a fresh `Environment` claims to know nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub power: Reading<Power>,
    pub disk: Reading<Disk>,
    pub clock: Reading<Clock>,
    pub skew: Reading<Skew>,
    pub fonts: Reading<InstalledFonts>,
    pub processes: Reading<RunningProcesses>,
    pub cameras: Reading<Cameras>,
    pub network: Reading<Network>,
    pub displays: Reading<Displays>,
    pub notifications: Reading<Notifications>,
    pub audio: Reading<Audio>,
    pub expected: Expectation,
    /// What this build was compiled for, so a remedy can name the right menu.
    /// Not a reading: it is never missing and never measured.
    pub platform: Platform,
}

impl Environment {
    /// Builder entry point, for tests and for callers assembling readings from
    /// somewhere other than [`crate::probe`] — a remote machine, a recorded
    /// session, a replayed bug report.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_power(mut self, power: Reading<Power>) -> Self {
        self.power = power;
        self
    }

    pub fn with_disk(mut self, disk: Reading<Disk>) -> Self {
        self.disk = disk;
        self
    }

    pub fn with_clock(mut self, clock: Reading<Clock>) -> Self {
        self.clock = clock;
        self
    }

    pub fn with_skew(mut self, skew: Reading<Skew>) -> Self {
        self.skew = skew;
        self
    }

    pub fn with_fonts(mut self, fonts: Reading<InstalledFonts>) -> Self {
        self.fonts = fonts;
        self
    }

    pub fn with_processes(mut self, processes: Reading<RunningProcesses>) -> Self {
        self.processes = processes;
        self
    }

    pub fn with_cameras(mut self, cameras: Reading<Cameras>) -> Self {
        self.cameras = cameras;
        self
    }

    pub fn with_network(mut self, network: Reading<Network>) -> Self {
        self.network = network;
        self
    }

    pub fn with_displays(mut self, displays: Reading<Displays>) -> Self {
        self.displays = displays;
        self
    }

    pub fn with_notifications(mut self, notifications: Reading<Notifications>) -> Self {
        self.notifications = notifications;
        self
    }

    pub fn with_audio(mut self, audio: Reading<Audio>) -> Self {
        self.audio = audio;
        self
    }

    pub fn expecting(mut self, expected: Expectation) -> Self {
        self.expected = expected;
        self
    }

    /// Says which platform these readings came from.
    ///
    /// The seam the three platform checks are tested through: a Linux runner
    /// builds an environment `on(Platform::Windows)` and asserts the remedy
    /// names Focus assist, which is the only way those branches are ever
    /// exercised anywhere but the one runner they run on.
    pub fn on(mut self, platform: Platform) -> Self {
        self.platform = platform;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_environment_knows_nothing() {
        // The default has to be "not measured" rather than "fine", or a caller
        // who forgets to fill a field ships a green report about a machine
        // nobody looked at.
        let environment = Environment::new();

        assert!(!environment.power.is_known());
        assert!(!environment.disk.is_known());
        assert!(!environment.clock.is_known());
        assert!(!environment.skew.is_known());
        assert!(!environment.fonts.is_known());
        assert!(!environment.processes.is_known());
        assert!(!environment.cameras.is_known());
        assert!(!environment.network.is_known());
        assert!(!environment.displays.is_known());
        assert!(!environment.notifications.is_known());
        assert!(!environment.audio.is_known());
    }

    #[test]
    fn a_fresh_environment_does_not_claim_a_platform_either() {
        // A remedy naming System Settings on a machine nobody said was a Mac
        // sends a speaker looking for a menu that is not there.
        assert_eq!(Environment::new().platform, Platform::Unknown);
        assert_eq!(Environment::new().on(Platform::Windows).platform, Platform::Windows);
    }

    #[test]
    fn an_unavailable_reading_explains_itself() {
        let reading: Reading<Power> = Reading::unavailable("no battery interface on this platform");

        assert_eq!(reading.value(), None);
        assert_eq!(reading.reason(), Some("no battery interface on this platform"));
    }

    #[test]
    fn a_known_reading_has_no_reason_to_give() {
        let reading = Reading::known(Power::on_battery(80));

        assert!(reading.is_known());
        assert_eq!(reading.reason(), None);
    }

    #[test]
    fn an_expectation_is_empty_rather_than_missing() {
        // Nobody has to say what zone the talk is in. The absence is a fact the
        // clock check reports on, not a hole that breaks it.
        let expected = Expectation::default();

        assert!(expected.fonts.is_empty());
        assert_eq!(expected.venue_offset_minutes, None);
        assert_eq!(expected.camera_slides, 0, "a deck nobody described wants no camera");
    }

    #[test]
    fn an_expectation_takes_css_font_stacks_verbatim_from_a_theme() {
        // `theme.font_sans` is a CSS string; making the caller pre-parse it
        // would be one more place for the two to drift apart.
        let expected = Expectation::default().with_font_stack("sans", "Inter, system-ui");

        assert_eq!(expected.fonts.len(), 1);
        assert_eq!(expected.fonts[0].families, vec!["Inter", "system-ui"]);
    }

    #[test]
    fn the_builder_fills_one_reading_and_leaves_the_rest_unmeasured() {
        let environment = Environment::new().with_power(Reading::known(Power::on_mains(100)));

        assert!(environment.power.is_known());
        assert!(!environment.disk.is_known());
    }
}
