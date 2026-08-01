//! Slide-to-slide transitions, as cross-document view transitions.
//!
//! A slidx deck is one HTML document per slide and navigation is the browser
//! following a link. There is no client router to hand a transition to, and
//! adding one to get animation would cost the properties the multi-page shape
//! exists for: a shareable, indexable slide URL that renders with no
//! JavaScript.
//!
//! So the transition is CSS. [`@view-transition`][spec] tells the browser to
//! animate across a same-origin navigation itself, which is the only mechanism
//! that animates between two *documents*. Without this module the choice would
//! be a router — meaning a deck that needs JavaScript to advance — or no
//! transitions at all.
//!
//! Browsers that do not implement it ignore the at-rule and navigate
//! instantly. That is the correct fallback and needs no feature detection: an
//! instant cut is what a deck does today.
//!
//! [spec]: https://drafts.csswg.org/css-view-transitions-2/#view-transition-rule
//!
//! ## Which slide's CSS decides
//!
//! The pseudo-element tree lives in the *arriving* document, so a slide's
//! `transition:` describes how it comes on screen, not how it leaves. Both
//! documents must opt in for anything to run, which means a slide set to
//! `none` is also an instant cut on the way out of it. That is worth knowing
//! before wondering why one transition in a deck does nothing.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::theme::Theme;

/// How one slide gives way to the next.
///
/// Four, and deliberately not a menu. Every entry here has to hold frame rate
/// on venue hardware and survive [`Transition::moves`] being cancelled for a
/// viewer who asked for less motion — which rules out anything that spins,
/// zooms, or flips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Transition {
    /// An instant cut. No animation is emitted at all.
    #[default]
    None,
    /// The slides cross-fade.
    Fade,
    /// The arriving slide slides in over a stationary one, like a card dealt
    /// on top of the deck.
    Slide,
    /// Both slides move in lockstep, the arriving one pushing the other off.
    Push,
}

impl Transition {
    /// Every transition, in the order they are offered to an author.
    pub const ALL: [Self; 4] = [Self::None, Self::Fade, Self::Slide, Self::Push];

    /// Reads a `transition:` token.
    ///
    /// Returns `None` for anything unrecognised rather than falling back
    /// silently, so a caller can tell a typo from a deliberate `none`. A deck
    /// that quietly stops animating because of a misspelling is a bug the
    /// author finds on stage, if at all.
    pub fn parse(token: &str) -> Option<Self> {
        match token.trim().to_ascii_lowercase().as_str() {
            // `off` and `false` are what people write when they mean "not
            // this one", and the step vocabulary already accepts them.
            "none" | "off" | "false" => Some(Self::None),
            "fade" => Some(Self::Fade),
            "slide" => Some(Self::Slide),
            "push" => Some(Self::Push),
            _ => None,
        }
    }

    /// Stable token used in frontmatter and in generated class names.
    pub fn as_token(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Fade => "fade",
            Self::Slide => "slide",
            Self::Push => "push",
        }
    }

    /// The compact name shown wherever a person chooses or previews this transition.
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "Cut",
            Self::Fade => "Fade",
            Self::Slide => "Slide",
            Self::Push => "Push",
        }
    }

    /// One sentence explaining the visible relationship between the two slides.
    pub fn description(self) -> &'static str {
        match self {
            Self::None => "Instant, with no captured animation.",
            Self::Fade => "Blend softly between the two slides.",
            Self::Slide => "Bring the next slide over the current one.",
            Self::Push => "Move both slides together to show progression.",
        }
    }

    /// Whether this transition translates the whole slide across the screen.
    ///
    /// Full-screen movement is the specific thing that triggers vestibular
    /// symptoms — nausea and vertigo, not annoyance. These are the transitions
    /// [`render`] has to cancel under `prefers-reduced-motion`.
    pub fn moves(self) -> bool {
        matches!(self, Self::Slide | Self::Push)
    }
}

/// The offered tokens, as help text. Derived so it cannot drift from [`Transition::ALL`].
///
/// Read by `slidx_dialect`, which is where a token outside the set is reported.
/// This module used to carry a `resolve` that reported it and had no caller
/// anywhere: the shell resolves with `parse().unwrap_or_default()`, so a typo was
/// an instant cut and the author concluded their browser lacked the feature.
/// Reporting now happens in the one place that reports on a deck's dialect, and
/// this file keeps the vocabulary — which is the part nobody should restate.
pub fn vocabulary() -> String {
    let tokens: Vec<String> =
        Transition::ALL.iter().map(|kind| format!("`{}`", kind.as_token())).collect();

    match tokens.split_last() {
        Some((last, rest)) => format!("{}, or {last}", rest.join(", ")),
        None => String::new(),
    }
}

/// Renders the stylesheet for a transition.
///
/// Emitted into every slide document of a deck. [`Transition::None`] renders
/// nothing at all — not a zero-duration animation — because the opt-in is what
/// makes the browser capture and composite two page snapshots, and a deck that
/// wants an instant cut should not pay for that work.
pub fn render(theme: &Theme, transition: Transition) -> String {
    if transition == Transition::None {
        return String::new();
    }

    let mut css = String::with_capacity(1024);

    let _ = writeln!(css, ":root {{");
    let _ = writeln!(css, "  --slidx-transition-duration: {}ms;", theme.motion.transition_ms);
    let _ =
        writeln!(css, "  --slidx-transition-easing: {};", theme.motion.transition_easing.as_css());
    let _ = writeln!(css, "}}\n");

    // The whole mechanism, and the reason no script is involved. It also
    // answers the first-paint question for free: a view transition needs two
    // documents, one captured on the way out and one on the way in. Opening a
    // deck has no outgoing document, so no `::view-transition-old` pseudo-
    // element is created and none of the rules below match anything. There is
    // no "suppress on load, re-enable a frame later" flag to get wrong,
    // because there is nothing to suppress.
    let _ = writeln!(css, "@view-transition {{");
    let _ = writeln!(css, "  navigation: auto;");
    let _ = writeln!(css, "}}\n");

    animations(&mut css, transition);
    reduced_motion(&mut css, theme);

    css
}

fn animations(css: &mut String, transition: Transition) {
    match transition {
        // Already returned from `render`; listed so a new kind cannot be added
        // without deciding what it animates.
        Transition::None => {}

        Transition::Fade => {
            // The blend mode the browser sets by default is right here: two
            // snapshots cross-fading with `normal` compositing dip to the page
            // background halfway through, which reads as a flash.
            pair(css, "slidx-transition-fade-out", "slidx-transition-fade-in", None);
            keyframes(css, "slidx-transition-fade-out", "to", "opacity: 0;");
            keyframes(css, "slidx-transition-fade-in", "from", "opacity: 0;");
        }

        Transition::Slide => {
            // `animation: none` holds the outgoing slide still at full opacity
            // while the arriving one covers it. Only one thing moves, which is
            // half the visual weight of a push for the same sense of order.
            let _ = writeln!(css, "::view-transition-old(root) {{");
            let _ = writeln!(css, "  animation: none;");
            let _ = writeln!(css, "  mix-blend-mode: normal;");
            let _ = writeln!(css, "}}\n");
            let _ = writeln!(css, "::view-transition-new(root) {{");
            let _ = writeln!(css, "  animation: {};", shorthand("slidx-transition-slide-in"));
            let _ = writeln!(css, "  mix-blend-mode: normal;");
            let _ = writeln!(css, "}}\n");
            keyframes(css, "slidx-transition-slide-in", "from", "transform: translateX(100%);");
        }

        Transition::Push => {
            pair(
                css,
                "slidx-transition-push-out",
                "slidx-transition-push-in",
                Some("mix-blend-mode: normal;"),
            );
            keyframes(css, "slidx-transition-push-out", "to", "transform: translateX(-100%);");
            keyframes(css, "slidx-transition-push-in", "from", "transform: translateX(100%);");
        }
    }
}

/// Rules for the outgoing and arriving snapshots.
///
/// `extra` carries `mix-blend-mode: normal` for the transitions that overlap
/// two opaque slides. The browser blends the snapshots with `plus-lighter` by
/// default, which is what makes a cross-fade hold a steady brightness — and
/// what blows the overlap out to white when two full-opacity slides pass over
/// each other.
fn pair(css: &mut String, out_name: &str, in_name: &str, extra: Option<&str>) {
    for (pseudo, name) in [("old", out_name), ("new", in_name)] {
        let _ = writeln!(css, "::view-transition-{pseudo}(root) {{");
        let _ = writeln!(css, "  animation: {};", shorthand(name));
        if let Some(extra) = extra {
            let _ = writeln!(css, "  {extra}");
        }
        let _ = writeln!(css, "}}\n");
    }
}

/// An `animation` shorthand timed from theme tokens.
fn shorthand(name: &str) -> String {
    format!("{name} var(--slidx-transition-duration) var(--slidx-transition-easing) both")
}

fn keyframes(css: &mut String, name: &str, stop: &str, declaration: &str) {
    let _ = writeln!(css, "@keyframes {name} {{");
    let _ = writeln!(css, "  {stop} {{");
    let _ = writeln!(css, "    {declaration}");
    let _ = writeln!(css, "  }}");
    let _ = writeln!(css, "}}\n");
}

/// Cancels the movement for a viewer who asked for less of it.
///
/// This is not a nicety. A full-screen push is the textbook trigger for
/// vestibular symptoms, and the person it affects has already told the browser
/// so — ignoring that is the same class of decision as ignoring a contrast
/// requirement.
///
/// What is left is an opacity change and nothing else: no transform appears
/// anywhere in this block, for any kind. A cross-fade is kept rather than a
/// hard cut because a slide changing with no signal at all is easy to miss,
/// and opacity carries no motion vector to be sick over. A deck that wants a
/// true cut asks for `transition: none`.
fn reduced_motion(css: &mut String, theme: &Theme) {
    let _ = writeln!(css, "@media (prefers-reduced-motion: reduce) {{");
    let _ = writeln!(css, "  :root {{");
    let _ = writeln!(css, "    --slidx-transition-duration: {}ms;", theme.motion.reduced_ms());
    let _ = writeln!(css, "  }}\n");

    for (pseudo, name, stop) in
        [("old", "slidx-transition-cut-out", "to"), ("new", "slidx-transition-cut-in", "from")]
    {
        let _ = writeln!(css, "  ::view-transition-{pseudo}(root) {{");
        // Linear, not the theme's curve: a theme may ease with a spring, whose
        // overshoot past 1 clamps on opacity and shows as a plateau.
        let _ =
            writeln!(css, "    animation: {name} var(--slidx-transition-duration) linear both;");
        let _ = writeln!(css, "    mix-blend-mode: plus-lighter;");
        let _ = writeln!(css, "  }}\n");

        let _ = writeln!(css, "  @keyframes {name} {{");
        let _ = writeln!(css, "    {stop} {{");
        let _ = writeln!(css, "      opacity: 0;");
        let _ = writeln!(css, "    }}");
        let _ = writeln!(css, "  }}\n");
    }

    let _ = writeln!(css, "}}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin;

    fn css(transition: Transition) -> String {
        render(&builtin::minimal(), transition)
    }

    /// The `prefers-reduced-motion` block, brace-matched out of the stylesheet.
    fn reduced_block(css: &str) -> String {
        let start = css.find("@media (prefers-reduced-motion: reduce)").expect("no reduce block");
        let mut depth = 0usize;

        for (offset, character) in css[start..].char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return css[start..start + offset + 1].to_string();
                    }
                }
                _ => {}
            }
        }

        panic!("unterminated reduce block in:\n{css}");
    }

    #[test]
    fn every_kind_round_trips_through_its_token() {
        for kind in Transition::ALL {
            assert_eq!(Transition::parse(kind.as_token()), Some(kind));
        }
    }

    #[test]
    fn tokens_are_the_lowercase_spelling_an_author_types() {
        for kind in Transition::ALL {
            let token = kind.as_token();
            assert!(
                token.chars().all(|c| c.is_ascii_lowercase()),
                "`{token}` is not what someone writes in YAML"
            );
        }
    }

    #[test]
    fn every_choice_explains_itself_in_the_ui() {
        for kind in Transition::ALL {
            assert!(!kind.name().is_empty(), "{} has no name", kind.as_token());
            assert!(!kind.description().is_empty(), "{} has no description", kind.as_token());
        }
    }

    #[test]
    fn parsing_forgives_case_and_stray_whitespace() {
        // YAML happily carries a trailing space, and nobody should lose a
        // transition to one.
        assert_eq!(Transition::parse("  Fade "), Some(Transition::Fade));
        assert_eq!(Transition::parse("PUSH"), Some(Transition::Push));
    }

    #[test]
    fn the_spellings_of_off_all_mean_no_transition() {
        for token in ["none", "off", "false"] {
            assert_eq!(Transition::parse(token), Some(Transition::None), "`{token}`");
        }
    }

    #[test]
    fn an_unknown_token_is_reported_rather_than_silently_accepted() {
        // Returning `None` is what lets the caller tell a typo from a
        // deliberate `none`; a fallback baked in here would lose that.
        assert_eq!(Transition::parse("slide-left"), None);
        assert_eq!(Transition::parse("dissolve"), None);
        assert_eq!(Transition::parse(""), None);
    }

    #[test]
    fn the_vocabulary_names_every_transition_on_offer() {
        // It is help text in somebody's terminal, and a transition missing from
        // it is one they will never find out exists.
        let help = vocabulary();

        for kind in Transition::ALL {
            assert!(help.contains(kind.as_token()), "help omits `{}`", kind.as_token());
        }
    }

    #[test]
    fn the_default_is_no_transition() {
        // A deck gains motion by asking for it, never by upgrading slidx.
        assert_eq!(Transition::default(), Transition::None);
    }

    #[test]
    fn moving_kinds_are_the_ones_reduced_motion_has_to_cancel() {
        assert!(Transition::Slide.moves());
        assert!(Transition::Push.moves());
        assert!(!Transition::Fade.moves(), "opacity is not a vestibular trigger");
        assert!(!Transition::None.moves());
    }

    #[test]
    fn no_transition_renders_no_stylesheet_at_all() {
        // Not a zero-duration animation: the opt-in itself costs two page
        // snapshots and a composited overlay.
        assert!(css(Transition::None).is_empty());
    }

    #[test]
    fn every_animated_kind_opts_in_to_cross_document_navigation() {
        for kind in [Transition::Fade, Transition::Slide, Transition::Push] {
            let css = css(kind);
            assert!(css.contains("@view-transition"), "{} does not opt in", kind.as_token());
            assert!(css.contains("navigation: auto;"), "{} does not opt in", kind.as_token());
        }
    }

    #[test]
    fn each_kind_produces_a_distinct_animation() {
        // Three names for one effect would be a worse offer than one name.
        let rendered: Vec<String> =
            [Transition::Fade, Transition::Slide, Transition::Push].map(css).to_vec();

        for (index, one) in rendered.iter().enumerate() {
            for other in &rendered[index + 1..] {
                assert_ne!(one, other, "two kinds render identically");
            }
        }
    }

    #[test]
    fn a_fade_animates_opacity_and_moves_nothing() {
        let css = css(Transition::Fade);

        assert!(css.contains("opacity: 0;"));
        assert!(!css.contains("translateX"), "a fade that moves is a slide");
    }

    #[test]
    fn a_slide_moves_only_the_arriving_page() {
        let css = css(Transition::Slide);

        assert!(css.contains("animation: none;"), "the outgoing slide stays put");
        assert!(css.contains("slidx-transition-slide-in"));
        assert_eq!(css.matches("translateX").count(), 1, "only one thing should move");
    }

    #[test]
    fn a_push_moves_both_pages_in_opposite_directions() {
        let css = css(Transition::Push);

        assert!(css.contains("translateX(-100%)"), "the outgoing slide leaves");
        assert!(css.contains("translateX(100%)"), "the arriving slide enters");
    }

    #[test]
    fn overlapping_slides_composite_opaquely() {
        // The browser blends the two snapshots additively by default, which is
        // right for a cross-fade and turns the overlap of two opaque slides
        // white.
        for kind in [Transition::Slide, Transition::Push] {
            assert!(
                css(kind).contains("mix-blend-mode: normal;"),
                "{} would blow out where the slides overlap",
                kind.as_token()
            );
        }
    }

    #[test]
    fn reduced_motion_cancels_every_movement() {
        for kind in [Transition::Fade, Transition::Slide, Transition::Push] {
            let block = reduced_block(&css(kind));

            assert!(!block.contains("translate"), "{} still moves under reduce", kind.as_token());
            assert!(!block.contains("scale"), "{} still scales under reduce", kind.as_token());
        }
    }

    #[test]
    fn reduced_motion_keeps_an_opacity_change_rather_than_nothing() {
        // A slide that changes with no signal at all is easy to miss; opacity
        // is the one cue that carries no motion.
        let block = reduced_block(&css(Transition::Push));
        assert!(block.contains("opacity: 0;"));
    }

    #[test]
    fn reduced_motion_is_honoured_by_every_kind_that_renders() {
        for kind in [Transition::Fade, Transition::Slide, Transition::Push] {
            assert!(
                css(kind).contains("@media (prefers-reduced-motion: reduce)"),
                "{} ignores the preference",
                kind.as_token()
            );
        }
    }

    #[test]
    fn reduced_motion_never_runs_longer_than_the_theme_asked_for() {
        // A theme that already moves fast keeps its own timing; the ceiling
        // only ever shortens.
        for theme in builtin::all() {
            assert!(
                theme.motion.reduced_ms() <= theme.motion.transition_ms,
                "{} slows down when asked to calm down",
                theme.id
            );
        }
    }

    #[test]
    fn durations_come_from_theme_tokens_rather_than_the_stylesheet() {
        let calm = render(&builtin::editorial(), Transition::Push);
        let brisk = render(&builtin::terminal(), Transition::Push);

        assert!(calm.contains("--slidx-transition-duration: 320ms;"));
        assert!(brisk.contains("--slidx-transition-duration: 140ms;"));
        assert_ne!(calm, brisk, "a theme that cannot change the timing has no token");
    }

    #[test]
    fn every_duration_in_the_stylesheet_is_a_variable() {
        // One literal survives per block — the token definition itself. Any
        // other `ms` is a duration a theme cannot reach.
        for kind in [Transition::Fade, Transition::Slide, Transition::Push] {
            let css = css(kind);
            assert_eq!(
                css.matches("ms;").count(),
                css.matches("--slidx-transition-duration:").count(),
                "{} hard-codes a duration:\n{css}",
                kind.as_token()
            );
        }
    }

    #[test]
    fn the_easing_comes_from_the_theme_token() {
        let css = render(&builtin::editorial(), Transition::Fade);
        assert!(css.contains(&format!(
            "--slidx-transition-easing: {};",
            builtin::editorial().motion.transition_easing.as_css()
        )));
    }

    #[test]
    fn nothing_animates_on_the_first_paint_of_a_deck() {
        // The guarantee is structural: every animation is bound to a
        // `::view-transition-*` pseudo-element, and those exist only while the
        // browser is holding two documents. Opening a deck has one. An
        // animation on a real selector would run on load, which is the bug
        // this excludes.
        for kind in [Transition::Fade, Transition::Slide, Transition::Push] {
            let mut selector = String::new();

            for line in css(kind).lines().map(str::trim) {
                if line.ends_with('{') {
                    selector = line.to_string();
                } else if line.starts_with("animation:") {
                    assert!(
                        selector.contains("::view-transition-"),
                        "`{}` animates `{selector}`, which exists on first paint",
                        kind.as_token()
                    );
                }
            }
        }
    }

    #[test]
    fn every_animation_named_is_a_keyframe_that_exists() {
        // A misspelt animation name is silently inert in CSS: the slide simply
        // appears, and nothing reports why.
        for kind in [Transition::Fade, Transition::Slide, Transition::Push] {
            let css = css(kind);
            let defined: Vec<&str> = css
                .match_indices("@keyframes ")
                .map(|(at, _)| css[at + 11..].split_whitespace().next().unwrap())
                .collect();

            for used in css.match_indices("animation: ").filter_map(|(at, _)| {
                css[at + 11..]
                    .split_whitespace()
                    .next()
                    .map(|name| name.trim_end_matches(';'))
                    .filter(|name| *name != "none")
            }) {
                assert!(defined.contains(&used), "`{used}` is animated but never defined");
            }
        }
    }

    #[test]
    fn animation_names_are_namespaced_to_slidx() {
        // A deck embeds third-party CSS; an unprefixed `fade-in` collides.
        for kind in [Transition::Fade, Transition::Slide, Transition::Push] {
            let css = css(kind);
            for (at, _) in css.match_indices("@keyframes ") {
                let name = css[at + 11..].split_whitespace().next().unwrap();
                assert!(name.starts_with("slidx-"), "`{name}` is not namespaced");
            }
        }
    }

    #[test]
    fn braces_balance_for_every_kind_and_every_theme() {
        for theme in builtin::all() {
            for kind in Transition::ALL {
                let css = render(&theme, kind);
                assert_eq!(
                    css.matches('{').count(),
                    css.matches('}').count(),
                    "unbalanced braces in {} / {}:\n{css}",
                    theme.id,
                    kind.as_token()
                );
            }
        }
    }

    #[test]
    fn every_built_in_theme_renders_every_kind() {
        for theme in builtin::all() {
            for kind in [Transition::Fade, Transition::Slide, Transition::Push] {
                assert!(
                    !render(&theme, kind).is_empty(),
                    "{} rendered nothing for {}",
                    theme.id,
                    kind.as_token()
                );
            }
        }
    }
}
