//! One key, for the moment a live demo dies.
//!
//! `slidx_render::shell` already ships both sides of a demo in the markup with
//! `preload="auto"`, laid out at the same size, and `slidx_core::demo` says why:
//! switching is **one attribute write**. No element is created, no file is
//! requested, no promise has to resolve. This is the half that was missing —
//! something a speaker can press.
//!
//! # Why it is inline rather than the runtime's `createDemoSwitch`
//!
//! Because of when it is used. The moment a fallback becomes necessary is the
//! moment the network stopped working, and a switch that first has to fetch a
//! module is a second thing that fails, at the same instant, for the same
//! reason. `scripts/budget.mjs` holds the figure that says so: *javascript a
//! slide with no steps fetches: 0*.
//!
//! That is decisive rather than a preference, and it is why the module in
//! `packages/runtime` cannot be the thing that ships here. Two implementations
//! of one rule is what this repository forbids, so the reader that survives
//! there is the presenter's — it asks whether a recording has buffered, which
//! is a different question asked from a different window.
//!
//! # What the projector says about it
//!
//! `ready()` in the runtime's module reports what the browser actually buffered
//! rather than what the markup asked for, and that is worth knowing *before* a
//! demo dies — after it, the speaker presses the key and finds out. But the
//! element is in this document and the presenter view is another one: its
//! next-slide preview is deliberately inert, and a presenter page that fetched
//! the same file would be proving something true about the wrong machine.
//!
//! So the projector says, on the channel the two windows already share. One
//! more message kind, unsequenced — a position needs ordering because two
//! windows both move and a stale one must not win, and a readiness report is a
//! fact about one element where the latest is always the truest.
//!
//! Only ever about the slide on screen, because a projector has one slide
//! loaded. "The fallback for slide 7" is a question nothing can answer until
//! slide 7 is up — which is still before the demo dies, and that is the whole
//! window this reading is useful in.
//!
//! # What it costs a deck without a demo
//!
//! Nothing. The script is emitted only for a slide that declares a fallback,
//! which is the same condition `shell::demo_markup` uses to emit the recording.
//! A deck with no demo is byte-identical to one built before this existed.
//!
//! # `d`, and why that key
//!
//! It is what `DEFAULT_BINDINGS` in `packages/runtime/src/keymap.ts` already
//! says. Two key tables that disagree is the shape of #255 and #257, and both
//! cost a talk — so a new binding takes the name the table already gave it even
//! while that table is the one nobody can see yet.

use slidx_core::{demo::DEMO_ATTRIBUTE, Slide};

/// The key binding, for a slide that has something to switch to.
///
/// Empty for a slide with no demo, and empty for a demo with no recording. The
/// second is not an oversight: `slidx_lint` has already reported the missing
/// fallback, and a key bound to a switch with nowhere to go is a key a speaker
/// presses again, harder, on stage.
pub fn script(slide: &Slide) -> String {
    let Some(demo) = &slide.demo else { return String::new() };
    if !demo.has_fallback() {
        return String::new();
    }

    // Written compactly for the same reason `navigation::script` is: every byte
    // is on a page an audience downloads, and the reasoning belongs in this
    // module rather than in a comment a room waits for.
    format!(
        r#"<script>
(() => {{
const figure = document.querySelector("[{DEMO_ATTRIBUTE}]");
const video = figure && figure.querySelector("video");
if (!video) return;
// The presenter view cannot read this element: its preview is inert, and a
// page that fetched the same file would be proving something true about the
// wrong machine. So the projector says.
const mirror = typeof BroadcastChannel === "undefined" ? null : new BroadcastChannel("slidx:slidx");
const me = "" + Math.random();
const tell = () => mirror?.postMessage({{
  type: "demo",
  demo: {{ ready: video.readyState >= 2, side: figure.getAttribute("{DEMO_ATTRIBUTE}") }},
  sequence: 1,
  from: me,
}});
// `preload="auto"` is advisory, and this is the one asset in a deck where a
// browser's judgement about whether the fetch is worth it is wrong.
if (video.readyState === 0) video.load();
// On load, and again the moment the browser has enough to play. A speaker
// arriving at the slide learns the answer without waiting for it.
tell();
video.addEventListener("loadeddata", tell);
const show = (side) => {{
  // Re-showing the side already on screen would rewind the recording to a
  // frame the speaker has talked past.
  if (figure.getAttribute("{DEMO_ATTRIBUTE}") === side) return;
  figure.setAttribute("{DEMO_ATTRIBUTE}", side);
  // Autoplay is refused for reasons that have nothing to do with this deck.
  // The recording is on screen either way, with controls, so a refused play
  // costs one click rather than the demo.
  if (side === "fallback") Promise.resolve(video.play()).catch(() => {{}});
  // Left running, a hidden recording keeps decoding behind the live demo.
  else video.pause();
  tell();
}};
addEventListener("keydown", (event) => {{
  if (event.key !== "d" || event.defaultPrevented) return;
  if (event.metaKey || event.ctrlKey || event.altKey) return;
  if (event.target?.closest?.("input,textarea,select,[contenteditable]")) return;
  event.preventDefault();
  show(figure.getAttribute("{DEMO_ATTRIBUTE}") === "fallback" ? "live" : "fallback");
}});
}})();
</script>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::demo::Demo;

    fn slide_with(demo: Option<Demo>) -> Slide {
        Slide { demo, ..Slide::default() }
    }

    #[test]
    fn a_slide_with_no_demo_pays_nothing_for_this() {
        assert_eq!(script(&slide_with(None)), "");
    }

    #[test]
    fn a_demo_with_no_recording_binds_no_key() {
        // The linter already reported the missing fallback. A key that visibly
        // does nothing is a key a speaker presses again, harder, on stage.
        let demo = Demo { live: "https://example.test".into(), ..Demo::default() };

        assert_eq!(script(&slide_with(Some(demo))), "");
    }

    #[test]
    fn the_key_is_the_one_the_binding_table_already_named() {
        let demo = Demo {
            live: "https://example.test".into(),
            fallback: Some("./demo.mp4".into()),
            poster: None,
        };
        let emitted = script(&slide_with(Some(demo)));

        assert!(emitted.contains(r#"event.key !== "d""#), "{emitted}");
    }

    #[test]
    fn switching_is_one_attribute_write_and_fetches_nothing() {
        // The property the whole feature rests on. A switch that requested
        // anything would be requesting it at the moment the network died.
        let demo = Demo {
            live: "https://example.test".into(),
            fallback: Some("./demo.mp4".into()),
            poster: None,
        };
        let emitted = script(&slide_with(Some(demo)));

        assert!(emitted.contains(&format!(r#"figure.setAttribute("{DEMO_ATTRIBUTE}", side)"#)));
        assert!(!emitted.contains("import "), "the switch fetches a module:\n{emitted}");
        assert!(!emitted.contains("fetch("), "the switch makes a request:\n{emitted}");
    }

    #[test]
    fn the_recording_is_primed_rather_than_left_to_the_browser() {
        let demo = Demo {
            live: "https://example.test".into(),
            fallback: Some("./demo.mp4".into()),
            poster: None,
        };
        let emitted = script(&slide_with(Some(demo)));

        assert!(emitted.contains("video.load()"), "{emitted}");
    }

    #[test]
    fn the_projector_says_whether_the_recording_will_play() {
        // The presenter view cannot read this element — its preview is inert by
        // design — so the window that has it says. Before the demo dies, which
        // is the only window the answer is useful in.
        let demo = Demo {
            live: "https://example.test".into(),
            fallback: Some("./demo.mp4".into()),
            poster: None,
        };
        let emitted = script(&slide_with(Some(demo)));

        assert!(emitted.contains(r#"type: "demo""#), "{emitted}");
        assert!(emitted.contains("video.readyState >= 2"), "{emitted}");
        assert!(emitted.contains(r#"video.addEventListener("loadeddata", tell)"#), "{emitted}");
    }

    #[test]
    fn it_says_again_when_the_side_changes() {
        // "Ready" and "showing the recording" are different sentences, and the
        // presenter has no other way to learn which one is true.
        let demo = Demo {
            live: "https://example.test".into(),
            fallback: Some("./demo.mp4".into()),
            poster: None,
        };
        let emitted = script(&slide_with(Some(demo)));
        let switch = emitted.split("const show").nth(1).expect("no switch");

        assert!(switch.contains("tell();"), "the switch says nothing:\n{switch}");
    }

    #[test]
    fn a_key_meant_for_a_field_or_a_shortcut_is_left_alone() {
        let demo = Demo {
            live: "https://example.test".into(),
            fallback: Some("./demo.mp4".into()),
            poster: None,
        };
        let emitted = script(&slide_with(Some(demo)));

        assert!(emitted.contains("event.metaKey || event.ctrlKey || event.altKey"), "{emitted}");
        assert!(emitted.contains("input,textarea,select,[contenteditable]"), "{emitted}");
    }
}
