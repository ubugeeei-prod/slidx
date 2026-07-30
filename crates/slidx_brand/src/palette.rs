//! The palette, mixed rather than picked.
//!
//! # The referent
//!
//! slidx is about **ink on paper that has to survive being light on a wall.** A
//! deck is written as a document and then thrown at a screen by a machine that
//! cannot emit black, and everything the linter models is about that second half.
//!
//! So the brand is one pigment: a **blue-black ink**, the colour a document gets
//! written in. That is the whole palette. There is no second hue anywhere in it.
//!
//! - **signal** is the ink at full strength — the only colour allowed to *mean*
//!   something. A link, an accent rule, the pages in the mark.
//! - **paper**, **ink**, **muted** and **line** are the *same pigment as a
//!   wash*: identical hue, a tenth of the chroma, at four lightnesses.
//!
//! That is what answers "why this neutral and not a warmer one". The neutral is
//! not a choice beside the signal — it *is* the signal, diluted. A warm grey
//! would mean two pigments, and the palette would stop being one idea. The
//! consequence is visible if you look for it: paper is very faintly cool, and ink
//! is a blue-black rather than a neutral black.
//!
//! # Nothing here is a hex literal
//!
//! Four numbers go in — a hue, a chroma, a wash fraction, and a lightness per
//! role — and the hexes come out through [`crate::ink`]. That is deliberate
//! beyond tidiness: a palette written as ten hex literals is a palette nobody can
//! argue with, and it is also exactly the shape a borrowed framework scale
//! arrives in. `scripts/check-borrowed.mjs` fails the build if a shipped colour
//! matches a framework default, and this module structurally cannot produce one.
//!
//! # The dark scheme is not a reflection, and that is a finding
//!
//! The light ladder is five lightnesses, one per job. The obvious way to get the
//! dark one is to reflect it, and the reflection *nearly* works — which is worse
//! than not working, because it looks finished.
//!
//! It fails on the projector model. A projector cannot emit black: the darkest
//! pixel is whatever light the room is already putting on the screen. That
//! ambient floor is added to both colours, and adding a constant to two small
//! luminances destroys their ratio far faster than adding it to two large ones.
//! So a dark scheme in a bright room loses much more contrast than its light twin
//! does, and needs to be *more* separated than the reflection, not equally.
//!
//! Rather than nudge two numbers until they looked right, the reflection is the
//! starting point and [`slidx_lint`] decides where each stop lands: each dark
//! role moves away from the paper, half a percent of lightness at a time, until
//! it clears its own floor in a bright room with a margin. The audit decides, not
//! a constant here — the same arrangement `TypeScale::code_factor` documents on
//! its own side.

use serde::{Deserialize, Serialize};
use slidx_lint::{projected_contrast_ratio, ProjectorProfile, Rgba, Surface, TextRole, TextSample};

use crate::ink::Oklch;

/// The one hue, in OKLCh degrees.
///
/// 258 is a blue-black ink: unmistakably blue, a shade cooler than the sRGB blue
/// primary at 264, and far from the 300-plus where blue becomes violet. Chosen
/// for the referent rather than for the number, and stated here so the next
/// person changes a pigment instead of ten hexes.
pub const HUE: f64 = 258.0;

/// The ink at full strength.
///
/// As strong as the hue gets at signal lightness while staying inside sRGB.
/// Pushing further does not produce a stronger blue — it produces the same blue
/// with the chroma clamped away, and a number that lies about what shipped.
pub const SIGNAL_CHROMA: f64 = 0.154;

/// How much of the pigment survives in a neutral.
///
/// A tenth. Enough that paper, ink and the signal read as one family; little
/// enough that ink is not a blue. Below about a twentieth the relationship stops
/// being visible at all and the palette becomes grey-plus-a-blue, which is the
/// thing it exists not to be.
pub const WASH: f64 = 0.10;

/// Where the dark ladder starts before the linter moves it.
///
/// The light ladder reflected: `dark = REFLECTION - light`. Above 1.0 rather than
/// at it, because sRGB has more usable separation above middle grey than below —
/// a dark scheme reflected about exactly 0.5 comes out too dark to distinguish
/// its own stops before contrast is even considered.
const REFLECTION: f64 = 1.19;

/// How much better than the floor a solved dark stop has to be.
///
/// A stop that lands exactly on 4.5:1 is one rounding away from failing, and the
/// audit would then depend on the order of two floating-point operations.
const MARGIN: f64 = 1.05;

/// Lightness step the solver moves by. Below what an 8-bit channel resolves, so
/// the answer is the first lightness that works rather than a coarse one near it.
const STEP: f64 = 0.005;

/// Which variant of the brand is in use.
///
/// Deliberately its own type rather than a reuse of [`slidx_theme::Scheme`]: a
/// brand asset is not a slide, and coupling them would mean a deck theme could
/// not gain a scheme without changing the brand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scheme {
    Light,
    Dark,
}

impl Scheme {
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub const ALL: [Self; 2] = [Self::Light, Self::Dark];
}

/// The five jobs a colour can have here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// What the brand is drawn on.
    Paper,
    /// Words.
    Ink,
    /// Secondary words: captions, attributions.
    Muted,
    /// The one colour that carries meaning.
    Signal,
    /// A hairline.
    Line,
}

impl Role {
    pub const ALL: [Self; 5] = [Self::Paper, Self::Ink, Self::Muted, Self::Signal, Self::Line];

    pub fn as_token(self) -> &'static str {
        match self {
            Self::Paper => "paper",
            Self::Ink => "ink",
            Self::Muted => "muted",
            Self::Signal => "signal",
            Self::Line => "line",
        }
    }

    /// Lightness in the light scheme, and why it is that.
    fn light_lightness(self) -> f64 {
        match self {
            // Near white, not white. At full separation the edges of a glyph
            // halate, which makes text harder to read rather than easier — the
            // high-contrast deck theme stops short of it for the same reason.
            Self::Paper => 0.985,
            // A tenth of the ladder below paper: present as a division, nowhere
            // near reading as a bar.
            Self::Line => 0.885,
            // As light as secondary text can be while still clearing its floor
            // in a bright room. Any lighter and a caption stops being readable
            // from the back of the room it is projected in.
            Self::Muted => 0.505,
            // Just below the middle. The signal has to work as text *and* as a
            // large flat fill with paper on top of it, and both directions are
            // measured.
            Self::Signal => 0.42,
            // Near black, not black, for the halation reason above.
            Self::Ink => 0.22,
        }
    }

    /// The contrast floor this role answers to, as the linter sets it.
    ///
    /// `muted` is a caption and `line` is not text at all — a hairline held to a
    /// text floor would be a border loud enough to read as a rule.
    fn floor(self) -> Option<f64> {
        match self {
            Self::Paper | Self::Line => None,
            Self::Muted => Some(3.0),
            Self::Ink | Self::Signal => Some(4.5),
        }
    }

    /// How the linter judges this role's size and contrast.
    fn text_role(self) -> TextRole {
        match self {
            Self::Muted => TextRole::Caption,
            _ => TextRole::Body,
        }
    }

    /// Chroma before the gamut clamp: full strength for the signal, a wash for
    /// everything else.
    fn chroma(self) -> f64 {
        match self {
            Self::Signal => SIGNAL_CHROMA,
            _ => SIGNAL_CHROMA * WASH,
        }
    }
}

/// The five colours, resolved to sRGB.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    pub paper: Rgba,
    pub ink: Rgba,
    pub muted: Rgba,
    pub signal: Rgba,
    pub line: Rgba,
}

impl Palette {
    pub fn get(&self, role: Role) -> Rgba {
        match role {
            Role::Paper => self.paper,
            Role::Ink => self.ink,
            Role::Muted => self.muted,
            Role::Signal => self.signal,
            Role::Line => self.line,
        }
    }
}

/// The light scheme: the ladder as stated.
pub fn light() -> Palette {
    let mixed = |role: Role| Oklch::new(role.light_lightness(), role.chroma(), HUE).to_rgba();

    Palette {
        paper: mixed(Role::Paper),
        ink: mixed(Role::Ink),
        muted: mixed(Role::Muted),
        signal: mixed(Role::Signal),
        line: mixed(Role::Line),
    }
}

/// The dark scheme: the reflection, moved until the linter is satisfied.
///
/// Paper and line have no floor to clear, so they are the reflection exactly.
/// The three that carry words are solved against the paper they will sit on.
pub fn dark() -> Palette {
    let reflected = |role: Role| REFLECTION - role.light_lightness();
    let mixed = |lightness: f64, role: Role| Oklch::new(lightness, role.chroma(), HUE).to_rgba();

    let paper = mixed(reflected(Role::Paper), Role::Paper);
    let solve = |role: Role| separated(paper, reflected(role), role);

    Palette {
        paper,
        ink: solve(Role::Ink),
        muted: solve(Role::Muted),
        signal: solve(Role::Signal),
        line: mixed(reflected(Role::Line), Role::Line),
    }
}

/// Walks a role away from the paper until it clears its floor in a bright room.
///
/// Away, not towards: on a dark scheme every role that carries words is lighter
/// than the paper, so more separation means a higher lightness. The bright room
/// is the profile that binds — it is the one where a dark scheme loses most, and
/// a stop that survives it survives the others.
fn separated(paper: Rgba, start: f64, role: Role) -> Rgba {
    let Some(floor) = role.floor() else {
        return Oklch::new(start, role.chroma(), HUE).to_rgba();
    };

    let target = floor * MARGIN;
    let mut lightness = start;

    while lightness <= 1.0 {
        let candidate = Oklch::new(lightness, role.chroma(), HUE).to_rgba();
        let ratio = projected_contrast_ratio(candidate, paper, ProjectorProfile::BrightRoom);

        if ratio >= target {
            return candidate;
        }
        lightness += STEP;
    }

    // Unreachable for the shipped ladder, and asserted so. Returning white
    // rather than panicking keeps a third-party palette from taking a deck down
    // if this is ever reused for one.
    Oklch::new(1.0, role.chroma(), HUE).to_rgba()
}

pub fn of(scheme: Scheme) -> Palette {
    match scheme {
        Scheme::Light => light(),
        Scheme::Dark => dark(),
    }
}

impl Palette {
    /// Describes the palette to the deck linter.
    ///
    /// Two surfaces, because the brand has two backgrounds anything is drawn on:
    /// paper, and a signal fill with paper on top of it. A filled button is the
    /// second, and a brand that failed there would fail in public on the first
    /// page of its own documentation.
    pub fn surfaces(&self, scheme: Scheme, base_px: f64) -> Vec<Surface> {
        let name = |part: &str| format!("brand / {} / {part}", scheme.as_token());

        let mut paper = Surface::new(name("paper"), self.paper);
        for role in Role::ALL {
            if role.floor().is_none() {
                continue;
            }
            paper = paper.with_text(TextSample::new(
                role.text_role(),
                self.get(role),
                base_px,
                format!("brand.{}", role.as_token()),
            ));
        }

        // The mark's pages are the signal on paper, judged as body text on
        // purpose: a favicon at 16 pixels is smaller than any glyph the model
        // covers, so the shape needs at least as much separation as a word,
        // never less.
        paper = paper.with_text(TextSample::new(
            TextRole::Body,
            self.signal,
            base_px,
            "brand.mark.pages",
        ));

        let filled = Surface::new(name("signal"), self.signal).with_text(TextSample::new(
            TextRole::Body,
            self.paper,
            base_px,
            "brand.onSignal",
        ));

        vec![paper, filled]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_neutral_is_the_signal_diluted() {
        // The claim the whole palette rests on: one pigment, not a grey scale
        // beside a blue. If a neutral ever gained a hue of its own this fails.
        for role in Role::ALL {
            if role == Role::Signal {
                continue;
            }
            assert_eq!(
                role.chroma(),
                SIGNAL_CHROMA * WASH,
                "{} is not a wash of the signal",
                role.as_token()
            );
        }
    }

    #[test]
    fn the_palette_has_exactly_one_hue() {
        // Stated as a property of the mixing rather than checked on the output,
        // because the gamut clamp legitimately changes chroma and never hue.
        for role in Role::ALL {
            let mixed = Oklch::new(role.light_lightness(), role.chroma(), HUE);
            assert_eq!(mixed.h, HUE, "{} left the family", role.as_token());
        }
    }

    #[test]
    fn the_neutrals_carry_the_signals_hue_into_the_output() {
        // The visible consequence of the wash: paper is faintly cool and ink is
        // a blue-black. A palette whose neutrals came out perfectly grey would
        // mean the wash was too faint to be a decision.
        for palette in [light(), dark()] {
            for (name, color) in
                [("paper", palette.paper), ("ink", palette.ink), ("line", palette.line)]
            {
                assert!(color.b > color.r, "{name} came out neutral: {}", color.to_hex());
            }
        }
    }

    #[test]
    fn the_signal_is_the_only_role_a_reader_would_call_coloured() {
        let chromatic = |color: Rgba| i16::from(color.b) - i16::from(color.r) > 40;

        for palette in [light(), dark()] {
            assert!(chromatic(palette.signal), "there is no signal");

            for (name, color) in
                [("paper", palette.paper), ("ink", palette.ink), ("muted", palette.muted)]
            {
                assert!(!chromatic(color), "{name} competes with the signal");
            }
        }
    }

    #[test]
    fn the_signal_is_a_blue_and_not_a_violet() {
        // The failure this palette was built to correct. A violet has red and
        // blue together with green behind both; this has to stay unambiguously
        // blue-dominant with red the smallest channel.
        for palette in [light(), dark()] {
            let signal = palette.signal;
            assert!(
                signal.b > signal.g && signal.g > signal.r,
                "the signal is not a blue: {}",
                signal.to_hex()
            );
        }
    }

    #[test]
    fn neither_scheme_reaches_full_black_on_full_white() {
        // Halation at full separation makes text harder to read, not easier.
        assert_ne!(light().paper, Rgba::WHITE);
        assert_ne!(light().ink, Rgba::BLACK);
        assert_ne!(dark().paper, Rgba::BLACK);
        assert_ne!(dark().ink, Rgba::WHITE);
    }

    #[test]
    fn the_dark_scheme_is_darker() {
        assert!(dark().paper.relative_luminance() < light().paper.relative_luminance());
        assert!(dark().ink.relative_luminance() > light().ink.relative_luminance());
    }

    #[test]
    fn the_dark_ladder_ends_up_more_separated_than_the_reflection() {
        // The finding the module documents. If a reflection had been enough,
        // this test would fail and the solver would be dead weight.
        let paper = dark().paper;
        let mut moved = Vec::new();

        for role in [Role::Ink, Role::Muted, Role::Signal] {
            let reflection =
                Oklch::new(REFLECTION - role.light_lightness(), role.chroma(), HUE).to_rgba();
            let solved = dark().get(role);

            if solved != reflection {
                moved.push(role.as_token());
            }

            assert!(
                solved.relative_luminance() >= reflection.relative_luminance(),
                "{} moved towards the paper rather than away",
                role.as_token()
            );
            assert!(
                projected_contrast_ratio(solved, paper, ProjectorProfile::BrightRoom)
                    >= role.floor().unwrap() * MARGIN,
                "{} does not clear its floor with a margin",
                role.as_token()
            );
        }

        assert!(
            !moved.is_empty(),
            "the reflection was already enough, so the solver is measuring nothing"
        );
    }

    #[test]
    fn the_light_ladder_descends_from_paper_to_ink() {
        let ordered = [Role::Paper, Role::Line, Role::Muted, Role::Signal, Role::Ink];
        let lightnesses: Vec<f64> = ordered.iter().map(|role| role.light_lightness()).collect();

        for pair in lightnesses.windows(2) {
            assert!(pair[0] > pair[1], "the ladder is not ordered: {lightnesses:?}");
        }
    }

    #[test]
    fn a_role_with_no_floor_is_left_where_the_reflection_put_it() {
        // Paper is the background and a hairline is not text. Solving either
        // against a contrast floor would be answering a question nobody asked.
        assert_eq!(Role::Paper.floor(), None);
        assert_eq!(Role::Line.floor(), None);
        assert_eq!(
            dark().line,
            Oklch::new(REFLECTION - Role::Line.light_lightness(), Role::Line.chroma(), HUE)
                .to_rgba()
        );
    }

    #[test]
    fn every_role_that_carries_words_is_described_to_the_linter() {
        // A role absent from this list is a role nobody checks — the gap
        // `slidx_theme::palette::pairs` exists to close on its own side.
        let described: Vec<String> = light()
            .surfaces(Scheme::Light, 17.0)
            .into_iter()
            .flat_map(|surface| surface.text)
            .map(|sample| sample.origin)
            .collect();

        assert_eq!(
            described,
            vec!["brand.ink", "brand.muted", "brand.signal", "brand.mark.pages", "brand.onSignal"]
        );
    }

    #[test]
    fn a_filled_signal_is_checked_as_its_own_background() {
        let surfaces = dark().surfaces(Scheme::Dark, 17.0);
        let filled = surfaces.iter().find(|surface| surface.name.ends_with("signal")).unwrap();

        assert_eq!(filled.background, dark().signal);
    }

    #[test]
    fn the_palette_is_deterministic() {
        // The tokens are committed. A solver that returned a different answer on
        // a second call would rewrite every generated file for nothing.
        assert_eq!(light(), light());
        assert_eq!(dark(), dark());
    }

    #[test]
    fn roles_round_trip_through_their_tokens() {
        for role in Role::ALL {
            assert!(!role.as_token().is_empty());
        }
        assert_eq!(Role::ALL.len(), 5);
        assert_eq!(Scheme::Light.as_token(), "light");
    }
}
