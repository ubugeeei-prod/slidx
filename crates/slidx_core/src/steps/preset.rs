//! Animation vocabulary.
//!
//! The presets mirror the entrance / emphasis / exit split that presentation
//! authors already know from desktop tools, so the frontmatter reads like the
//! menu they are used to picking from.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which phase of an element's life an effect belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum EffectKind {
    #[default]
    Entrance,
    Emphasis,
    Exit,
}

/// A named animation. Each preset maps to one CSS keyframe set in the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum EffectPreset {
    /// No motion at all. Respected verbatim when a deck opts out of animation.
    None,

    // Entrance
    #[default]
    Fade,
    FlyIn,
    Wipe,
    Zoom,
    Split,
    Grow,
    Float,
    Typewriter,
    Draw,

    // Emphasis
    Pulse,
    Shake,
    Spin,
    ColorPulse,
    Underline,

    // Exit
    FadeOut,
    FlyOut,
    WipeOut,
    ZoomOut,
    Shrink,
}

impl EffectPreset {
    /// The preset used when the author names an action but not an animation.
    pub fn default_for(kind: EffectKind) -> Self {
        match kind {
            EffectKind::Entrance => Self::Fade,
            EffectKind::Emphasis => Self::Pulse,
            EffectKind::Exit => Self::FadeOut,
        }
    }

    /// The phase this preset naturally belongs to.
    ///
    /// Used to warn when a deck asks for, say, an exit animation on a reveal.
    pub fn kind(self) -> EffectKind {
        match self {
            Self::Pulse | Self::Shake | Self::Spin | Self::ColorPulse | Self::Underline => {
                EffectKind::Emphasis
            }
            Self::FadeOut | Self::FlyOut | Self::WipeOut | Self::ZoomOut | Self::Shrink => {
                EffectKind::Exit
            }
            _ => EffectKind::Entrance,
        }
    }

    /// Stable token used for the runtime's CSS class and data attributes.
    pub fn as_token(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Fade => "fade",
            Self::FlyIn => "fly-in",
            Self::Wipe => "wipe",
            Self::Zoom => "zoom",
            Self::Split => "split",
            Self::Grow => "grow",
            Self::Float => "float",
            Self::Typewriter => "typewriter",
            Self::Draw => "draw",
            Self::Pulse => "pulse",
            Self::Shake => "shake",
            Self::Spin => "spin",
            Self::ColorPulse => "color-pulse",
            Self::Underline => "underline",
            Self::FadeOut => "fade-out",
            Self::FlyOut => "fly-out",
            Self::WipeOut => "wipe-out",
            Self::ZoomOut => "zoom-out",
            Self::Shrink => "shrink",
        }
    }

    /// Whether the preset animates transform/opacity only.
    ///
    /// Compositor-only effects stay smooth on projector hardware; the linter
    /// uses this to flag decks that will judder in a conference room.
    pub fn is_compositor_only(self) -> bool {
        !matches!(self, Self::Typewriter | Self::Draw | Self::ColorPulse | Self::Underline)
    }
}

/// Direction an effect travels from or towards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

impl Origin {
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Center => "center",
        }
    }
}

/// Timing curve for an effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum Easing {
    Linear,
    Ease,
    EaseIn,
    #[default]
    EaseOut,
    EaseInOut,
    Spring,
}

impl Easing {
    /// CSS timing function. `spring` is approximated with a cubic-bezier so the
    /// runtime never needs a physics loop on the presentation hot path.
    pub fn as_css(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Ease => "ease",
            Self::EaseIn => "ease-in",
            Self::EaseOut => "cubic-bezier(0.22, 1, 0.36, 1)",
            Self::EaseInOut => "ease-in-out",
            Self::Spring => "cubic-bezier(0.34, 1.56, 0.64, 1)",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_report_their_natural_phase() {
        assert_eq!(EffectPreset::Fade.kind(), EffectKind::Entrance);
        assert_eq!(EffectPreset::Pulse.kind(), EffectKind::Emphasis);
        assert_eq!(EffectPreset::FadeOut.kind(), EffectKind::Exit);
    }

    #[test]
    fn each_action_kind_has_a_sensible_default_preset() {
        assert_eq!(EffectPreset::default_for(EffectKind::Entrance), EffectPreset::Fade);
        assert_eq!(EffectPreset::default_for(EffectKind::Emphasis), EffectPreset::Pulse);
        assert_eq!(EffectPreset::default_for(EffectKind::Exit), EffectPreset::FadeOut);
    }

    #[test]
    fn paint_heavy_presets_are_flagged_for_the_linter() {
        assert!(EffectPreset::Fade.is_compositor_only());
        assert!(!EffectPreset::Typewriter.is_compositor_only());
    }

    #[test]
    fn tokens_are_kebab_case_and_unique() {
        let presets = [
            EffectPreset::None,
            EffectPreset::Fade,
            EffectPreset::FlyIn,
            EffectPreset::Wipe,
            EffectPreset::Zoom,
            EffectPreset::Split,
            EffectPreset::Grow,
            EffectPreset::Float,
            EffectPreset::Typewriter,
            EffectPreset::Draw,
            EffectPreset::Pulse,
            EffectPreset::Shake,
            EffectPreset::Spin,
            EffectPreset::ColorPulse,
            EffectPreset::Underline,
            EffectPreset::FadeOut,
            EffectPreset::FlyOut,
            EffectPreset::WipeOut,
            EffectPreset::ZoomOut,
            EffectPreset::Shrink,
        ];

        let mut tokens: Vec<&str> = presets.iter().map(|preset| preset.as_token()).collect();
        let total = tokens.len();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(tokens.len(), total);
        assert!(tokens
            .iter()
            .all(|token| token.chars().all(|c| c.is_ascii_lowercase() || c == '-')));
    }

    #[test]
    fn easing_maps_to_css_timing_functions() {
        assert_eq!(Easing::Linear.as_css(), "linear");
        assert!(Easing::Spring.as_css().starts_with("cubic-bezier("));
        assert_eq!(Easing::default(), Easing::EaseOut);
    }
}
