//! The second opt-in a declared camera has always needed.
//!
//! `slidx_core::camera` says the shape of this and has said it since the
//! feature landed:
//!
//! > The declaration and the stream are two different things, and only the
//! > first of them is in the file. […] Nothing in this crate, and nothing the
//! > build emits from it, opens a device — that needs a second opt-in, from the
//! > speaker, at presentation time, and it lives in the runtime.
//!
//! It lived in the runtime and nothing called it, so **`camera:` in a deck
//! placed a tile that stayed empty forever**. `enterPresentation` was where the
//! second opt-in was supposed to be, and it is on the presenter view — a page
//! that has no tile, so it asked for a device with nowhere to put one and
//! reported `off` with a reason about the slide.
//!
//! # `c`, on the window the tile is on
//!
//! The gesture has to be on the audience window, because that is where the deck
//! rendered the tile. So it is a key on that page, bound only on a slide that
//! declares a camera — which keeps the gate exactly where the core module put
//! it. A deck with no `camera:` anywhere has no script, no import and no path
//! to a device, and that is still true of a page somebody opens from an
//! archive.
//!
//! A key rather than something on screen, and that is the deliberate part. A
//! button on the slide would be a control an audience can see and a speaker has
//! to point at; a key is a thing only the person at the keyboard knows about.
//! `slidx_render::keys` lists it, on the presenter view, and only for a deck
//! that has one.
//!
//! # Its own module, and why not the shared one
//!
//! `camera.ts` is 224 lines of device handling and error classification, and
//! folding it into the entry every deck downloads would put it in front of
//! every audience for the sake of the few decks that place one — which is the
//! thing #291 spent two changes undoing. So the plugin emits it as its own file
//! and only when a deck declares a camera.
//!
//! It is therefore a module import, which a `file://` deck cannot resolve. That
//! costs nothing real: `getUserMedia` needs a secure context, so a deck opened
//! off a USB stick could not have reached a camera by any route.

use slidx_core::Slide;

/// The binding, for a slide that has somewhere to put a stream.
pub fn script(slide: &Slide, camera_src: &str) -> String {
    if slide.camera.is_none() {
        return String::new();
    }

    format!(
        r#"<script type="module">
import {{ browserCameraEnvironment, startCamera }} from "{camera_src}";

// One session per page. Pressing the key again stops the device rather than
// opening a second one — a camera light that stays on after a talk is a light
// the speaker has to go and find.
let camera;

addEventListener("keydown", async (event) => {{
  if (event.key !== "c" || event.defaultPrevented) return;
  if (event.metaKey || event.ctrlKey || event.altKey) return;
  if (event.target?.closest?.("input,textarea,select,[contenteditable]")) return;
  event.preventDefault();

  if (camera) {{
    camera.stop();
    camera = undefined;
    return;
  }}

  // Never rejects: a refused permission, a device in use by another
  // application, and a browser with no camera API are all statuses written onto
  // the tile rather than errors thrown into a slide.
  camera = await startCamera(document, browserCameraEnvironment(window));
}});
</script>
"#
    )
}

#[cfg(test)]
mod tests {
    use slidx_core::{parse_deck, DeckParseOptions};

    use super::*;

    fn slide_of(source: &str) -> Slide {
        parse_deck(source, &DeckParseOptions::default()).slides.remove(0)
    }

    #[test]
    fn a_deck_that_places_no_camera_has_no_path_to_a_device() {
        // The gate `slidx_core::camera` describes, kept where it put it. A page
        // somebody opens from an archive has no script, no import, and nothing
        // that could ask.
        assert_eq!(script(&slide_of("# One\n"), "./camera.js"), "");
    }

    #[test]
    fn a_declared_camera_binds_the_key_the_list_names() {
        let emitted =
            script(&slide_of("---\nlayout: aside\ncamera: side\n---\n\n# Remote\n"), "./camera.js");

        assert!(emitted.contains(r#"event.key !== "c""#), "{emitted}");
        assert!(emitted.contains("startCamera"), "{emitted}");
    }

    #[test]
    fn pressing_it_again_stops_the_device() {
        // A camera light that stays on after a talk is a light the speaker has
        // to go and find, on somebody else's stage.
        let emitted =
            script(&slide_of("---\nlayout: aside\ncamera: side\n---\n\n# Remote\n"), "./camera.js");

        assert!(emitted.contains("camera.stop()"), "{emitted}");
    }

    #[test]
    fn a_key_meant_for_a_field_or_a_shortcut_is_left_alone() {
        let emitted =
            script(&slide_of("---\nlayout: aside\ncamera: side\n---\n\n# Remote\n"), "./camera.js");

        assert!(emitted.contains("event.metaKey || event.ctrlKey || event.altKey"), "{emitted}");
        assert!(emitted.contains("input,textarea,select,[contenteditable]"), "{emitted}");
    }

    #[test]
    fn it_imports_from_the_address_the_builder_supplies() {
        let emitted = script(
            &slide_of("---\nlayout: aside\ncamera: side\n---\n\n# Remote\n"),
            "/deck/camera.js",
        );

        assert!(emitted.contains(r#"from "/deck/camera.js""#), "{emitted}");
    }
}
