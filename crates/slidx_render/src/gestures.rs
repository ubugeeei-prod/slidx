//! The gestures that mean the same thing on every slide.
//!
//! A slide gets one of two scripts. `shell::stage_script` fires for a slide with
//! two or more frames and binds the runtime's key handler; `navigation::script`
//! fires for everything else. Both bind the arrows, and only the second bound
//! `f` and a swipe — so two boxes that are checked in the roadmap were true of
//! half a deck:
//!
//! - **A phone could not advance a staged slide.** The footer's links measure
//!   four pixels by three on a 375px screen, which is why the swipe exists.
//!   There it is not a convenience, it is the navigation.
//! - **`f` did nothing on one.** A deck stayed in a browser window with the
//!   chrome around it and no wake lock, on exactly the slides an author had put
//!   animation into.
//!
//! Every test passed, because the two emitters are tested separately and each
//! does what it says.
//!
//! # A swipe is a key press
//!
//! The interesting half. `f` is two browser calls and needs nothing from either
//! script, but a swipe has to advance a *step* on a staged slide and a *slide*
//! on an unstaged one — and the smallest thing that already knows the difference
//! is the key handler the page installed.
//!
//! So a swipe dispatches the `ArrowRight` a thumb is standing in for, and this
//! module needs to know nothing about staging. It also means the gesture can
//! never disagree with the key: there is one behaviour, reached two ways.
//!
//! # Why it is emitted twice over
//!
//! An unstaged slide gets [`body`] spliced into `navigation::script`'s own
//! closure, and a staged one gets [`script`], which is the same statements in a
//! closure of their own. One emitter either way; what differs is whether there
//! is already a script to join.
//!
//! That is not a style choice. `scripts/budget.mjs` holds a slide with no steps
//! to a stated number of inlined bytes, and a second `<script>` element with a
//! second closure and a second frame guard would spend sixty of them on a slide
//! that already had both.

use slidx_core::Slide;

/// The statements, for splicing into a script that already exists.
///
/// Assumes the caller's closure has already refused to run inside a frame. The
/// editor draws every slide in its outline as a live frame of its own page, so
/// a swipe there would move six thumbnails at once.
pub(crate) fn body() -> &'static str {
    r#"
addEventListener("keydown", (event) => {
  if (event.key !== "f" || event.defaultPrevented) return;
  if (event.metaKey || event.ctrlKey || event.altKey) return;
  event.preventDefault();
  if (document.fullscreenElement) document.exitFullscreen();
  else document.documentElement.requestFullscreen?.().then(() =>
    navigator.wakeLock?.request("screen").catch(() => {}));
});
let from = null;
addEventListener("touchstart", (event) => {
  from = event.touches.length === 1 && !event.target.closest?.("a,button,input,textarea,select")
    ? { x: event.touches[0].clientX, y: event.touches[0].clientY, at: event.timeStamp }
    : null;
}, { passive: true });
addEventListener("touchend", (event) => {
  const start = from;
  from = null;
  if (!start || event.changedTouches.length !== 1) return;
  const dx = event.changedTouches[0].clientX - start.x;
  const dy = event.changedTouches[0].clientY - start.y;
  if (event.timeStamp - start.at > 600 || Math.abs(dx) < 40 || Math.abs(dx) < Math.abs(dy) * 2) return;
  dispatchEvent(new KeyboardEvent("keydown", { key: dx < 0 ? "ArrowRight" : "ArrowLeft" }));
}, { passive: true });
"#
}

/// The same statements, for a slide whose script came from somewhere else.
///
/// A staged slide's script is a module, and a module is deferred: binding a
/// gesture there would leave the first seconds of a slide unable to answer one.
/// This is a classic script and runs where it is written.
pub(crate) fn script(slide: &Slide) -> String {
    // A staged slide is the one that had neither. An unstaged slide gets the
    // same statements from `navigation::script`, inside the closure it already
    // opened.
    if slide.timeline.frames().len() < 2 {
        return String::new();
    }

    format!("<script>\n(() => {{\nif (window.top !== window) return;{}}})();\n</script>\n", body())
}

#[cfg(test)]
mod tests {
    use slidx_core::{parse_deck, DeckParseOptions};

    use super::*;

    fn slide_of(source: &str) -> Slide {
        parse_deck(source, &DeckParseOptions::default()).slides.remove(0)
    }

    fn staged() -> Slide {
        slide_of("---\nsteps:\n  - reveal: point\n---\n\n# One\n\n[a point]{#point}\n")
    }

    #[test]
    fn a_staged_slide_answers_a_swipe_and_the_fullscreen_key() {
        // The defect this module exists for. Both of these shipped on an
        // unstaged slide and on no other, which made two roadmap boxes true of
        // half a deck.
        let emitted = script(&staged());

        assert!(emitted.contains("touchstart"), "no swipe on a staged slide:\n{emitted}");
        assert!(emitted.contains(r#"event.key !== "f""#), "no fullscreen key:\n{emitted}");
    }

    #[test]
    fn an_unstaged_slide_gets_these_from_the_script_it_already_has() {
        // Not a missing feature: `navigation::script` splices `body` into the
        // closure it already opened, which is sixty bytes cheaper on the one
        // slide `budget.mjs` measures.
        assert_eq!(script(&slide_of("# One\n")), "");
    }

    #[test]
    fn a_swipe_presses_the_key_a_thumb_is_standing_in_for() {
        // The whole reason this is one emitter. A swipe has to move a step on a
        // staged slide and a slide on an unstaged one, and the key handler each
        // page installed is the smallest thing that already knows which.
        let emitted = script(&staged());

        assert!(emitted.contains(r#"key: dx < 0 ? "ArrowRight" : "ArrowLeft""#), "{emitted}");
        assert!(emitted.contains("new KeyboardEvent"), "{emitted}");
    }

    #[test]
    fn a_gesture_inside_a_frame_moves_nothing() {
        // The editor draws every slide in its outline as a live frame of its
        // own page. One swipe there would otherwise move six thumbnails.
        assert!(script(&staged()).contains("if (window.top !== window) return;"));
    }
}
