//! Getting to the next slide.
//!
//! This was missing, and missing in the way the roadmap's own preamble
//! describes: the code existed, it was tested, and no audience could reach it.
//! `packages/runtime` has had `next`, `prev`, `first`, `last` and a key table
//! since M1. They were emitted into a slide only when it had **two or more
//! stops**, because that is the condition for shipping the stage. A slide with
//! no steps — a title slide, a quote, a full-bleed image, most of a deck —
//! received no script, no link, and no handler.
//!
//! So on the page an audience actually looks at, the right arrow did nothing,
//! there was nothing to click, and the presenter view's mirror broadcast into a
//! window with no listener. The speaker advanced and the projector did not.
//!
//! # Links first, and links always
//!
//! The fix is not a script. slidx already compiles a deck into one document per
//! slide with one URL each, and the web's own answer to "go to that document"
//! is an anchor. An anchor costs no JavaScript, works from a USB stick, works
//! when the script fails, is a tap target on a phone, is reachable by Tab, is
//! announced by a screen reader, and survives being printed.
//!
//! `<link rel="prev|next">` was already emitted into `<head>`, where no browser
//! has surfaced it for twenty years. The same two addresses in the footer are
//! navigation.
//!
//! Every path here is **relative**, which is what makes a deck work from a
//! file:// URL and out of a directory somebody moved. [`crate::url`] owns the
//! layout and this module never spells one itself.
//!
//! # And then keys, for the room
//!
//! Links answer clicking and tapping. They do not answer a presentation
//! clicker, which sends `PageDown`, and they do not answer the presenter view,
//! which drives the projector over a `BroadcastChannel`. Both of those are the
//! difference between a deck and a deck you can give a talk with.
//!
//! That needs a script, and this is the one place the front page's
//! "no JavaScript on a slide with no steps" had to give. What it gives is
//! stated exactly: **a few hundred bytes, inline, with no import, no module
//! graph, no framework, and no request to anything.** The offline guarantee,
//! the no-framework guarantee and the nothing-from-another-origin guarantee are
//! all untouched. What changed is a count that was zero and is now small, and
//! the roadmap says so rather than keeping a number that was true because the
//! feature did not work.
//!
//! The script computes nothing the markup does not already say. It reads the
//! two anchors and follows them; the destinations live in one place and it is
//! the place the reader can see.
//!
//! # A slide with steps
//!
//! Gets the links and not the script. Its stage already binds the keyboard and
//! the mirror through the runtime it loads, and a second handler would move the
//! deck two slides per press.
//!
//! It also *rebinds the links*, in `shell.rs`, because forward from a slide
//! mid-build is the next stop rather than the next slide. The href stays what
//! it is — that is the right answer for a reader with no JavaScript, for
//! anything that crawls the deck, and for opening a neighbour in a new tab —
//! and the stage intercepts the plain left click that means "advance".

use slidx_core::{Deck, Slide};

use crate::url;

/// Where slide `to` is, addressed from slide `from`.
fn href(from: u32, to: u32) -> String {
    format!("{}{}", url::up_to_root(from), url::slide_path(to))
}

/// The footer's slide counter, with the deck's two neighbours around it.
///
/// The edges are rendered as inert spans rather than omitted. A missing element
/// would move the counter sideways on the first and last slides, and a deck
/// whose chrome shifts under the audience reads as a page that is still
/// loading. They are also not links because there is nowhere for them to go,
/// and a disabled link is a promise the browser cannot keep.
pub fn links(deck: &Deck, slide: &Slide) -> String {
    let index = slide.index;
    let last = deck.slides.len().saturating_sub(1) as u32;

    // "Previous" rather than "Previous slide", because a slide with steps has
    // somewhere nearer to go and the stage rebinds these to it. One word that
    // is true in both modes beats two that are true in one — and the `<nav>`
    // around them is already named "Slides".
    let previous = step(index > 0, "prev", "Previous", "\u{2039}", || href(index, index - 1));
    let next = step(index < last, "next", "Next", "\u{203a}", || href(index, index + 1));

    format!(
        "<nav class=\"slidx-slide-nav\" aria-label=\"Slides\">\
         {previous}\
         <span class=\"slidx-slide-number\">{number} / {count}</span>\
         {next}\
         </nav>",
        number = index + 1,
        count = deck.slides.len(),
    )
}

fn step(
    reachable: bool,
    rel: &str,
    label: &str,
    glyph: &str,
    destination: impl Fn() -> String,
) -> String {
    if reachable {
        format!(
            "<a class=\"slidx-slide-step\" rel=\"{rel}\" href=\"{}\" aria-label=\"{label}\">\
             {glyph}</a>",
            destination()
        )
    } else {
        format!("<span class=\"slidx-slide-step\" aria-hidden=\"true\">{glyph}</span>")
    }
}

/// Keyboard and presenter mirroring for a slide with no stage to do it.
///
/// Empty for a staged slide, which gets both from the runtime the stage already
/// loads. Emitting it there would bind every key twice.
///
/// # Why it announces before it moves, and never on arrival
///
/// Both directions have to work. A clicker sends its keys to whichever window
/// is focused, and at a venue that is usually the projector rather than the
/// laptop — so a projector that only listened would leave the speaker's notes
/// behind the slide the room is looking at.
///
/// The mirror orders messages with a counter that is monotonic per sender, and
/// a page that has just navigated cannot carry one across the reload: every
/// message this script sends is sequence 1, under an identity of its own. That
/// is why the identity matters — see `MirrorMessage.from`, and the stage
/// failure that field is named after.
///
/// The position is sent **before** the navigation it describes rather than on
/// arrival, because a window that announces where it already is tells the room
/// nothing and spends a sequence number doing it. The staged path has the same
/// shape for the same reason: it sends from `subscribe`, which fires on the
/// move.
///
/// # A swipe
///
/// The footer's two glyphs are a poor target for a thumb, and a deck is read
/// on a phone far more often than it is given from one. A swipe is what a
/// person tries first there, and it costs nothing to answer: it moves the same
/// two links the keys do.
///
/// Bounded on all four sides, because a gesture that fires when it should not
/// is worse than one that never fires. One finger, so a pinch to zoom is not a
/// page turn. Under 600ms, so a slow drag to select text is not one either.
/// Forty pixels, so a tap that wandered is a tap. And twice as far across as
/// down, so a scroll that drifted stays a scroll.
///
/// Both listeners are `passive`, so a swipe never blocks the compositor: the
/// page keeps scrolling at sixty frames while the gesture is being decided,
/// and the decision is made after the finger lifts.
///
/// # What it refuses
///
/// **A slide that is not the window it would be presented from.** The editor's
/// outline draws every slide as a live frame of its own real page, and its
/// canvas is another one — so a listener on the shared mirror channel is a
/// listener in six windows at once, and one position sent anywhere dragged
/// every thumbnail onto the same slide. The keyboard is the same mistake in
/// the other direction: an arrow key inside the canvas belongs to the editor,
/// or to the caret.
///
/// The links are left alone, which is the whole reason this can be one line: a
/// deck embedded in somebody's page is still navigable by clicking, because an
/// anchor needs nothing from this script to work.
///
/// A destination that is the empty string, which is how `Home` on slide one
/// does nothing: the deck root addressed *from* the deck root is `""`, and
/// reloading the page an audience is already looking at is worse than ignoring
/// the key.
///
/// Every key that is not plainly a request to move: anything with a modifier,
/// anything typed into a field or a contenteditable, and anything another
/// handler has already claimed. A deck is a document, and `⌘←` means go back.
/// A modified or middle click on a neighbour is left to the browser, so opening
/// the next slide in a new tab still does that.
pub fn script(deck: &Deck, slide: &Slide) -> String {
    if slide.timeline.frames().len() >= 2 {
        return String::new();
    }

    // Written compactly on purpose: every byte here is on every slide of every
    // deck, and the reasoning belongs in this comment rather than in a comment
    // an audience downloads. `shell.rs` holds it to a stated size.
    format!(
        r#"<script>
(() => {{
if (window.top !== window) return;
const here = {index}, nav = ".slidx-slide-nav";
const me = "" + Math.random();
const mirror = typeof BroadcastChannel === "undefined" ? null : new BroadcastChannel("slidx:slidx");
const go = (to, href) => {{
  if (!href) return;
  mirror?.postMessage({{ type: "position", position: {{ slide: to, step: 0 }}, sequence: 1, from: me }});
  location.assign(href);
}};
const step = (rel) =>
  go(rel === "next" ? here + 1 : here - 1, document.querySelector(`${{nav}} a[rel="${{rel}}"]`)?.href);
const keys = {{ ArrowRight: "next", ArrowDown: "next", PageDown: "next", " ": "next", Enter: "next",
  ArrowLeft: "prev", ArrowUp: "prev", PageUp: "prev", Backspace: "prev" }};
addEventListener("keydown", (event) => {{
  if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) return;
  if (event.target?.closest?.("input,textarea,select,a,button,[contenteditable]")) return;
  const rel = keys[event.key];
  if (typeof rel === "string") {{ event.preventDefault(); step(rel); }}
  else if (event.key === "Home") {{ event.preventDefault(); go(0, "{root}"); }}
  else if (event.key === "End") {{ event.preventDefault(); go({last}, "{end}"); }}
}});
addEventListener("click", (event) => {{
  const link = event.target.closest?.(`${{nav}} a[rel]`);
  if (!link || event.button || event.metaKey || event.ctrlKey || event.shiftKey) return;
  event.preventDefault();
  step(link.rel);
}});
let from = null;
addEventListener("touchstart", (event) => {{
  from = event.touches.length === 1 && !event.target.closest?.("a,button,input,textarea,select")
    ? {{ x: event.touches[0].clientX, y: event.touches[0].clientY, at: event.timeStamp }}
    : null;
}}, {{ passive: true }});
addEventListener("touchend", (event) => {{
  const start = from;
  from = null;
  if (!start || event.changedTouches.length !== 1) return;
  const dx = event.changedTouches[0].clientX - start.x;
  const dy = event.changedTouches[0].clientY - start.y;
  if (event.timeStamp - start.at > 600 || Math.abs(dx) < 40 || Math.abs(dx) < Math.abs(dy) * 2) return;
  step(dx < 0 ? "next" : "prev");
}}, {{ passive: true }});

mirror?.addEventListener("message", (event) => {{
  const to = event.data?.position?.slide;
  if (typeof to !== "number" || to === here) return;
  location.assign(to === 0 ? "{up}" : "{up}" + (to + 1) + "/");
}});
}})();
</script>
"#,
        index = slide.index,
        last = deck.slides.len().saturating_sub(1),
        up = if slide.index == 0 { "./" } else { "../" },
        root = href(slide.index, 0),
        end = href(slide.index, deck.slides.len().saturating_sub(1) as u32),
    )
}

#[cfg(test)]
mod tests {
    use slidx_core::{parse_deck, DeckParseOptions};

    use super::*;

    fn deck_of(source: &str) -> Deck {
        parse_deck(source, &DeckParseOptions::default())
    }

    fn three_slides() -> Deck {
        deck_of("# One\n\n---\n\n# Two\n\n---\n\n# Three\n")
    }

    #[test]
    fn every_slide_can_be_left_without_a_script() {
        // The defect this module exists for. A deck whose middle slide has no
        // outgoing link is a deck that ends there for anyone who cannot run
        // JavaScript — which includes a printed page and a failed CDN.
        let deck = three_slides();

        for slide in &deck.slides {
            let markup = links(&deck, slide);
            assert!(markup.contains("<a "), "slide {} has no link at all", slide.index + 1);
        }
    }

    #[test]
    fn the_addresses_are_the_ones_the_deck_is_built_at() {
        let deck = three_slides();

        let first = links(&deck, &deck.slides[0]);
        assert!(first.contains(r#"rel="next" href="2/""#), "got: {first}");

        let middle = links(&deck, &deck.slides[1]);
        assert!(middle.contains(r#"rel="prev" href="../""#), "slide one is the deck root");
        assert!(middle.contains(r#"rel="next" href="../3/""#), "got: {middle}");
    }

    #[test]
    fn every_address_is_relative() {
        // What makes a deck work from a USB stick and out of a directory
        // somebody moved. An absolute path resolves against the filesystem
        // root over file://.
        let deck = three_slides();

        for slide in &deck.slides {
            let markup = links(&deck, slide);
            assert!(!markup.contains("href=\"/"), "absolute path on slide {}", slide.index + 1);
        }
    }

    #[test]
    fn the_ends_of_the_deck_are_inert_rather_than_missing() {
        let deck = three_slides();

        let first = links(&deck, &deck.slides[0]);
        assert!(!first.contains(r#"rel="prev""#), "there is nothing before slide one");
        assert!(first.matches("slidx-slide-step").count() == 2, "both sides keep their place");

        let last = links(&deck, &deck.slides[2]);
        assert!(!last.contains(r#"rel="next""#));
        assert!(last.matches("slidx-slide-step").count() == 2);
    }

    #[test]
    fn a_one_slide_deck_has_no_links_and_still_has_its_counter() {
        let deck = deck_of("# Only\n");
        let markup = links(&deck, &deck.slides[0]);

        assert!(!markup.contains("<a "));
        assert!(markup.contains("1 / 1"));
    }

    #[test]
    fn the_counter_still_reads_the_way_a_person_counts() {
        let deck = three_slides();
        assert!(links(&deck, &deck.slides[1]).contains("2 / 3"));
    }

    #[test]
    fn the_neighbours_are_named_for_something_that_cannot_see_the_glyph() {
        let deck = three_slides();
        let markup = links(&deck, &deck.slides[1]);

        assert!(markup.contains(r#"aria-label="Previous""#));
        assert!(markup.contains(r#"aria-label="Next""#));
        assert!(markup.contains(r#"aria-label="Slides""#));
    }

    #[test]
    fn a_slide_with_steps_gets_no_script_here() {
        // The stage binds keys and the mirror through the runtime. A second
        // handler would move the deck two slides per press.
        let deck = deck_of("---\nautoSteps: list\n---\n\n- one\n- two\n- three\n");
        let slide = &deck.slides[0];

        assert!(slide.timeline.frames().len() >= 2, "fixture must be staged");
        assert_eq!(script(&deck, slide), "");
    }

    #[test]
    fn a_slide_without_steps_binds_keys_and_the_mirror() {
        let deck = three_slides();
        let emitted = script(&deck, &deck.slides[1]);

        assert!(emitted.contains("keydown"));
        assert!(emitted.contains("BroadcastChannel"));
        assert!(emitted.contains("PageDown"), "a clicker sends page keys");
    }

    #[test]
    fn the_script_asks_for_nothing_from_anywhere() {
        // The offline guarantee. No import, no src, no fetch — the whole reason
        // this is inline rather than a module.
        let deck = three_slides();
        let emitted = script(&deck, &deck.slides[0]);

        assert!(!emitted.contains("import "), "a module import does not resolve over file://");
        assert!(!emitted.contains("src="));
        assert!(!emitted.contains("fetch("));
        assert!(!emitted.contains("://"), "nothing from another origin");
        assert!(!emitted.contains("type=\"module\""), "and no module graph to resolve");
    }

    #[test]
    fn the_script_reads_its_destinations_off_the_links() {
        // Not a second copy of where the slides are. One source of truth, and
        // it is the one a reader can see in the markup.
        let deck = three_slides();
        let emitted = script(&deck, &deck.slides[1]);

        assert!(emitted.contains(r#"a[rel="${rel}"]"#), "got: {emitted}");
    }

    #[test]
    fn a_key_named_after_something_on_object_prototype_is_not_a_direction() {
        // `keys[event.key]` is a plain object lookup, so `__proto__` and
        // `constructor` come back truthy. Nothing reaches a URL either way, but
        // a guard that reads as "did we recognise this key" should mean it.
        let deck = three_slides();
        let emitted = script(&deck, &deck.slides[0]);

        assert!(emitted.contains(r#"typeof rel === "string""#), "got: {emitted}");
    }

    #[test]
    fn it_stays_out_of_the_way_of_typing_and_of_the_browser() {
        let deck = three_slides();
        let emitted = script(&deck, &deck.slides[0]);

        assert!(emitted.contains("defaultPrevented"));
        assert!(emitted.contains("metaKey"), "cmd-left is the browser's, not the deck's");
        assert!(emitted.contains("[contenteditable]"));
    }

    #[test]
    fn a_phone_can_swipe_because_it_cannot_hit_the_links() {
        // Every length on a slide is a share of the slide, which is what makes
        // a deck scale as one piece — and what makes the footer's links four
        // pixels by three on a 375px phone. Measured in a browser, after a
        // comment in `crate::layout` claimed they cleared 44px.
        let deck = three_slides();
        let emitted = script(&deck, &deck.slides[1]);

        assert!(emitted.contains("touchstart"));
        assert!(emitted.contains("touchend"));
        assert!(emitted.contains(r#"{ passive: true }"#), "a swipe must not block the compositor");
    }

    #[test]
    fn a_swipe_is_bounded_on_all_four_sides() {
        // A gesture that fires when it should not is worse than one that never
        // fires: one finger so a pinch is not a page turn, under 600ms so a
        // slow selection is not, forty pixels so a wandering tap is not, and
        // twice as far across as down so a drifting scroll is not.
        let deck = three_slides();
        let emitted = script(&deck, &deck.slides[1]);

        assert!(emitted.contains("touches.length === 1"));
        assert!(emitted.contains("> 600"));
        assert!(emitted.contains("< 40"));
        assert!(emitted.contains("Math.abs(dy) * 2"));
    }

    #[test]
    fn a_swipe_that_starts_on_a_link_belongs_to_the_link() {
        let deck = three_slides();
        assert!(script(&deck, &deck.slides[1])
            .contains(r#"closest?.("a,button,input,textarea,select")"#));
    }

    #[test]
    fn a_slide_drawn_inside_a_frame_binds_nothing() {
        // The editor's outline is one live frame per slide and its canvas is
        // another. Every one of them ran this script, so a single position on
        // the mirror channel pulled all six onto the same slide — every preview
        // in the panel showing whatever the canvas was showing.
        let deck = three_slides();
        let emitted = script(&deck, &deck.slides[1]);

        assert!(emitted.contains("if (window.top !== window) return;"), "got: {emitted}");
        assert!(
            emitted.find("window.top").unwrap() < emitted.find("addEventListener").unwrap(),
            "the guard has to come before anything is bound"
        );
    }

    #[test]
    fn a_key_that_would_land_where_it_already_is_does_nothing() {
        // `Home` on slide one. The deck root addressed from the deck root is
        // the empty string, and `go` refuses a falsy address — so the key is
        // ignored rather than reloading the page under the audience.
        let deck = three_slides();
        let emitted = script(&deck, &deck.slides[0]);

        assert!(emitted.contains(r#"go(0, "")"#), "got: {emitted}");
        assert!(emitted.contains("if (!href) return;"));
    }

    #[test]
    fn home_and_end_reach_the_ends_of_the_deck() {
        let deck = three_slides();
        let emitted = script(&deck, &deck.slides[2]);

        assert!(emitted.contains(r#"go(0, "../")"#), "Home is the deck root");
        assert!(emitted.contains(r#"go(2, "../3/")"#), "End is the last slide");
    }
}
