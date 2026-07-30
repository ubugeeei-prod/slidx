//! What the machine is drawing on.
//!
//! Two facts that fail apart, which is why they are one reading and two checks.
//! How many screens there are and how big they are is readable nearly
//! everywhere. Whether the arrangement *mirrors* one screen onto another is
//! not — and mirroring is the one that ends something, because presenter view
//! needs a second screen to live on and a mirrored pair is one screen wearing
//! two cables.
//!
//! So mirroring is an `Option<bool>` *inside* a reading that is otherwise
//! present. "slidx could not tell" has to survive next to a resolution slidx
//! did read, and collapsing the whole reading to unavailable would throw away
//! the half that was measured.

use serde::{Deserialize, Serialize};

/// A width and a height, in whatever unit the field naming it says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Resolution {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// `2560x1080`, the way a display's own settings panel writes it.
    ///
    /// An `x` rather than a multiplication sign: this ends up on a terminal in
    /// a venue, and the worst console in the building renders ASCII.
    pub fn label(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }

    /// True when either side is zero — a platform that answered with a shape
    /// it does not actually have.
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// One screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Display {
    /// What the platform calls it, when it names one. A speaker with two
    /// screens needs to know which line is the projector.
    pub name: Option<String>,
    /// The panel's own pixels, as the platform reports them.
    pub pixels: Resolution,
    /// The size the desktop is drawn at, when the platform says so.
    ///
    /// `None` means the platform did not say, never "one point per pixel". A
    /// scaled laptop panel that reported its pixels and nothing else would
    /// otherwise look like an enormous unscaled screen.
    pub points: Option<Resolution>,
    pub primary: bool,
}

impl Display {
    pub fn new(width: u32, height: u32) -> Self {
        Self { name: None, pixels: Resolution::new(width, height), points: None, primary: false }
    }

    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Declares the size the desktop is drawn at, for a scaled panel.
    pub fn drawn_at(mut self, width: u32, height: u32) -> Self {
        self.points = Some(Resolution::new(width, height));
        self
    }

    pub fn primary(mut self) -> Self {
        self.primary = true;
        self
    }

    /// The area a deck actually gets, in points where the platform said and in
    /// pixels where it did not.
    pub fn drawn_size(&self) -> Resolution {
        self.points.unwrap_or(self.pixels)
    }

    /// Backing pixels per point, as a percentage. `None` where the platform
    /// named only one of the two sizes, since one number cannot imply a ratio.
    pub fn scale_percent(&self) -> Option<u16> {
        let points = self.points?;

        (!points.is_empty())
            .then(|| u16::try_from(self.pixels.width as u64 * 100 / points.width as u64).ok())
            .flatten()
    }

    /// The scale as a speaker says it — `2x`, `1.5x` — or `None` on a screen
    /// drawn one point per pixel, where printing `1x` on every ordinary monitor
    /// is noise in a line that has to be scanned.
    pub fn scale_label(&self) -> Option<String> {
        let percent = self.scale_percent().filter(|percent| *percent != 100)?;

        Some(match percent % 100 {
            0 => format!("{}x", percent / 100),
            _ => format!("{:.1}x", f32::from(percent) / 100.0),
        })
    }

    /// One line naming this screen for the report: the room the deck gets, and
    /// the scale it is drawn at where that is not one to one.
    pub fn label(&self) -> String {
        let mut size = self.drawn_size().label();

        if let Some(scale) = self.scale_label() {
            size = format!("{size} at {scale}");
        }

        match &self.name {
            Some(name) => format!("{name} {size}"),
            None => size,
        }
    }
}

/// Every screen attached, and how they are arranged.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Displays {
    screens: Vec<Display>,
    mirrored: Option<bool>,
}

impl Displays {
    /// Screens whose arrangement the platform would not say anything about.
    ///
    /// The starting point rather than a special case: a probe has to go out of
    /// its way to claim an arrangement, because claiming the wrong one is worse
    /// than claiming none.
    pub fn new(screens: impl IntoIterator<Item = Display>) -> Self {
        Self { screens: screens.into_iter().collect(), mirrored: None }
    }

    /// The platform said these screens show different things.
    pub fn extended(mut self) -> Self {
        self.mirrored = Some(false);
        self
    }

    /// The platform said these screens show the same thing.
    pub fn mirrored(mut self) -> Self {
        self.mirrored = Some(true);
        self
    }

    pub fn screens(&self) -> &[Display] {
        &self.screens
    }

    pub fn len(&self) -> usize {
        self.screens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.screens.is_empty()
    }

    /// Whether one screen is being shown on another. `None` where the platform
    /// will not say, which is not the same answer as "no".
    pub fn is_mirrored(&self) -> Option<bool> {
        self.mirrored
    }

    /// The smallest area any attached screen gives a deck.
    ///
    /// The worst case is the one worth reporting: a laptop panel is never the
    /// screen that loses the room, and the projector beside it might be.
    pub fn smallest(&self) -> Option<&Display> {
        self.screens.iter().filter(|screen| !screen.drawn_size().is_empty()).min_by_key(|screen| {
            let size = screen.drawn_size();
            (size.width as u64) * (size.height as u64)
        })
    }

    /// Every screen, named, for one line of a finding.
    pub fn labels(&self) -> Vec<String> {
        self.screens.iter().map(Display::label).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_arrangement_nobody_reported_is_neither_mirrored_nor_extended() {
        // The whole reason mirroring is an Option. Windows in duplicate mode
        // shows one logical screen, so "one screen" there cannot be told from
        // "two screens showing the same thing" — and reporting extended would
        // send a speaker on stage expecting presenter view to open.
        assert_eq!(Displays::new([Display::new(1920, 1080)]).is_mirrored(), None);
    }

    #[test]
    fn a_platform_that_did_say_is_recorded_either_way() {
        assert_eq!(Displays::new([Display::new(1920, 1080)]).mirrored().is_mirrored(), Some(true));
        assert_eq!(Displays::new([Display::new(1920, 1080)]).extended().is_mirrored(), Some(false));
    }

    #[test]
    fn a_scaled_panel_reports_the_area_a_deck_actually_gets() {
        // A laptop panel with 3024 pixels draws a deck at 1512 points wide. A
        // check that reasoned about the pixels would conclude the deck has
        // twice the room it has.
        let screen = Display::new(3024, 1964).drawn_at(1512, 982);

        assert_eq!(screen.drawn_size(), Resolution::new(1512, 982));
        assert_eq!(screen.scale_percent(), Some(200));
    }

    #[test]
    fn a_screen_that_named_only_its_pixels_has_no_scale_to_report() {
        // One number cannot imply a ratio, and guessing 1:1 would report a
        // scaled panel as an unscaled one twice its real size.
        let screen = Display::new(2560, 1080);

        assert_eq!(screen.scale_percent(), None);
        assert_eq!(screen.drawn_size(), Resolution::new(2560, 1080));
    }

    #[test]
    fn a_screen_reporting_a_zero_side_yields_no_scale_rather_than_dividing_by_it() {
        let screen = Display::new(2560, 1080).drawn_at(0, 0);

        assert_eq!(screen.scale_percent(), None);
    }

    #[test]
    fn a_labelled_screen_says_its_name_the_room_it_gives_and_its_scale() {
        // A speaker with two screens has to be able to tell which line is the
        // projector, and the scale is what tells a laptop panel from one.
        let screen = Display::new(3024, 1964).drawn_at(1512, 982).named("Color LCD");

        assert_eq!(screen.label(), "Color LCD 1512x982 at 2x");
    }

    #[test]
    fn an_unnamed_unscaled_screen_says_only_its_size() {
        assert_eq!(Display::new(1920, 1080).label(), "1920x1080");
    }

    #[test]
    fn a_screen_drawn_one_point_per_pixel_does_not_say_so() {
        // Every ordinary monitor is 1x. Printing it on each of them is noise in
        // a line whose only job is to be scanned.
        let screen = Display::new(2560, 1080).drawn_at(2560, 1080).named("LG");

        assert_eq!(screen.label(), "LG 2560x1080");
        assert_eq!(screen.scale_percent(), Some(100));
        assert_eq!(screen.scale_label(), None);
    }

    #[test]
    fn a_scale_that_is_not_a_whole_multiple_keeps_its_fraction() {
        // Windows at 150% and a scaled laptop mode both land here, and "1x"
        // would be wrong by half a screen.
        let screen = Display::new(2880, 1620).drawn_at(1920, 1080);

        assert_eq!(screen.scale_label(), Some("1.5x".to_string()));
    }

    #[test]
    fn a_resolution_is_written_the_way_a_settings_panel_writes_it() {
        assert_eq!(Resolution::new(1920, 1080).label(), "1920x1080");
        assert!(Resolution::new(1920, 1080).label().is_ascii());
    }

    #[test]
    fn the_smallest_screen_is_the_one_a_deck_has_least_room_on() {
        // The laptop panel is never the screen that loses the room. The
        // projector beside it is, so it is the one the check speaks about.
        let displays = Displays::new([
            Display::new(3024, 1964).drawn_at(1512, 982).named("Color LCD"),
            Display::new(1280, 720).named("EPSON"),
        ]);

        assert_eq!(displays.smallest().and_then(|s| s.name.clone()), Some("EPSON".to_string()));
    }

    #[test]
    fn a_screen_that_reported_no_size_at_all_is_not_the_smallest() {
        // A platform answering 0x0 has told us nothing, and letting nothing win
        // a minimum would make every machine look like the worst possible one.
        let displays = Displays::new([Display::new(0, 0), Display::new(1920, 1080)]);

        assert_eq!(displays.smallest().map(|s| s.pixels), Some(Resolution::new(1920, 1080)));
    }

    #[test]
    fn a_machine_with_no_screens_has_no_smallest_one() {
        assert!(Displays::default().smallest().is_none());
        assert!(Displays::default().is_empty());
    }

    #[test]
    fn every_screen_is_listed_for_the_report() {
        let displays =
            Displays::new([Display::new(1920, 1080).named("A"), Display::new(1280, 800)]);

        assert_eq!(displays.labels(), ["A 1920x1080", "1280x800"]);
        assert_eq!(displays.len(), 2);
    }
}
