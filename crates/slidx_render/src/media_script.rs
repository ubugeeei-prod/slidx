//! The second half a clip on a slide has always needed.
//!
//! `packages/runtime/src/media.ts` says the shape of this and has said it
//! since the feature landed: measure a clip while the deck is still on an
//! earlier slide, and attenuate playback towards a target so one loud file
//! does not arrive at full scale. It lived in the runtime and nothing called
//! it, so **a `<video>` on a slide played at whatever level the file had**.
//!
//! # Two pages, two jobs
//!
//! The reading belongs on the presenter view, beside the demo-fallback line.
//! The speaker is looking at this slide and needs to know about the next one
//! *before* they arrive — a clip that startles the room cannot be un-played.
//!
//! The gain belongs on the audience slide that holds the element. Setting
//! `volume` there is what a room actually hears; asserting that a function
//! was called is not.
//!
//! # Its own module, and why not the shared one
//!
//! The same reason as `camera.ts`. Folding the decoder into the entry every
//! staged slide downloads would put Web Audio in front of every audience for
//! the sake of the few decks that place a clip. So the plugin emits it as its
//! own file and only when a page imports it. A deck with no `<video>` and no
//! `<audio>` has no script, no import, and no path to a decoder.

use serde::Serialize;
use slidx_core::{Deck, Slide};

use crate::markdown::{render as render_markdown, MarkdownOptions};

/// A clip the page will actually play — not a demo fallback, not a sample
/// in a fenced code block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Clip {
    pub src: String,
    pub kind: &'static str,
}

/// The binding, for a slide that has a clip a room will hear.
///
/// `frame` is the markup already on the page, so a fenced `<video>` in a
/// code sample — escaped by the renderer — cannot open a path to the decoder,
/// and a demo fallback that the shell injected is skipped by class.
pub fn slide_script(frame: &str, media_src: &str) -> String {
    if clips_in(frame).is_empty() {
        return String::new();
    }

    format!(
        r#"<script type="module">
import {{ createMediaController, decodeLevels }} from "{media_src}";

const clips = [...document.querySelectorAll("video, audio")].filter(
  (node) => !node.classList.contains("slidx-demo-fallback") && (node.currentSrc || node.src),
);

const controller = createMediaController({{
  measure: (media) => decodeLevels(new URL(media.src, location.href).href),
}});

for (const clip of clips) {{
  await controller.prepare(clip);
}}

addEventListener("pagehide", () => {{
  for (const clip of clips) controller.release(clip);
}});
</script>
"#
    )
}

/// The reading, for a presenter page that can still warn before the room hears
/// the clip.
///
/// Next slide first: that is the one the speaker has not arrived at. This
/// slide as well, because the first page of a deck has no earlier presenter
/// to have spoken, and a one-slide deck with a clip is that case.
pub fn presenter_script(
    deck: &Deck,
    slide: &Slide,
    media_src: &str,
    markdown: &MarkdownOptions,
) -> String {
    let here = clips_of(slide, markdown);
    let next = deck
        .slides
        .get(slide.index as usize + 1)
        .map(|next| clips_of(next, markdown))
        .unwrap_or_default();

    if here.is_empty() && next.is_empty() {
        return String::new();
    }

    format!(
        r#"<script type="module">
import {{ measureClip, describeLevel }} from "{media_src}";

const here = {here};
const next = {next};
const line = document.querySelector("[data-slidx-clip-level]");
const audience = new URL("../", location.href);

async function read(clips) {{
  const parts = [];
  let status = "";
  for (const clip of clips) {{
    const report = await measureClip(new URL(clip.src, audience).href);
    parts.push(
      report.levels ? describeLevel(report.levels.peakDb) : (report.remedy ?? "could not measure"),
    );
    if (report.status === "too-loud") status = "too-loud";
    else if (status === "" && report.status !== "ok") status = report.status;
  }}
  return {{ parts, status }};
}}

async function report() {{
  if (!line) return;
  const chunks = [];
  let status = "";
  if (here.length) {{
    line.textContent = "this slide: measuring…";
    const reading = await read(here);
    chunks.push(`this slide: ${{reading.parts.join(" · ")}}`);
    status = reading.status;
  }}
  if (next.length) {{
    line.textContent = "next: measuring…";
    const reading = await read(next);
    chunks.push(`next: ${{reading.parts.join(" · ")}}`);
    if (reading.status === "too-loud") status = "too-loud";
    else if (status === "") status = reading.status;
  }}
  line.textContent = chunks.join(" · ");
  if (status) line.dataset.slidxClipStatus = status;
}}

report();
</script>
"#,
        here = serde_json::to_string(&here).expect("clip srcs are JSON strings"),
        next = serde_json::to_string(&next).expect("clip srcs are JSON strings"),
    )
}

fn clips_of(slide: &Slide, markdown: &MarkdownOptions) -> Vec<Clip> {
    clips_in(&render_markdown(&slide.content, markdown))
}

/// Every playable clip in a fragment of HTML.
pub fn clips_in(html: &str) -> Vec<Clip> {
    let lower = html.to_ascii_lowercase();
    let mut clips = Vec::new();
    let mut at = 0;

    while at < lower.len() {
        let rest = &lower[at..];
        let video = find_tag(rest, "video");
        let audio = find_tag(rest, "audio");
        let found = match (video, audio) {
            (None, None) => break,
            (Some(v), Some(a)) if v <= a => (v, "video"),
            (Some(_), Some(a)) => (a, "audio"),
            (Some(v), None) => (v, "video"),
            (None, Some(a)) => (a, "audio"),
        };
        let start = at + found.0;
        let after = &html[start..];
        let Some(gt) = after.find('>') else { break };
        let open = &after[..=gt];

        if is_demo_fallback(open) {
            at = start + gt + 1;
            continue;
        }

        if let Some(src) = attr(open, "src").filter(|src| !src.is_empty()) {
            clips.push(Clip { src, kind: found.1 });
        } else {
            let close = format!("</{}", found.1);
            let inner_lower = after.to_ascii_lowercase();
            let inner_end = inner_lower.find(&close).unwrap_or(after.len());
            let inner = if inner_end > gt + 1 { &after[gt + 1..inner_end] } else { "" };
            for src in source_srcs(inner) {
                clips.push(Clip { src, kind: found.1 });
            }
        }

        at = start + gt + 1;
    }

    clips
}

fn find_tag(lower: &str, name: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(rel) = lower[from..].find(&format!("<{name}")) {
        let at = from + rel;
        let next = at + 1 + name.len();
        let following = lower.as_bytes().get(next).copied().unwrap_or(b'>');
        if matches!(following, b'>' | b'/' | b' ' | b'\t' | b'\n' | b'\r') {
            return Some(at);
        }
        from = at + 1;
    }
    None
}

fn is_demo_fallback(open: &str) -> bool {
    attr(open, "class")
        .is_some_and(|class| class.split_whitespace().any(|name| name == "slidx-demo-fallback"))
}

fn source_srcs(inner: &str) -> Vec<String> {
    let lower = inner.to_ascii_lowercase();
    let mut srcs = Vec::new();
    let mut at = 0;

    while let Some(rel) = find_tag(&lower[at..], "source") {
        let start = at + rel;
        let after = &inner[start..];
        let Some(gt) = after.find('>') else { break };
        if let Some(src) = attr(&after[..=gt], "src").filter(|src| !src.is_empty()) {
            srcs.push(src);
        }
        at = start + gt + 1;
    }

    srcs
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let needle = format!("{name}=");
    let mut from = 0;

    while let Some(rel) = lower[from..].find(&needle) {
        let at = from + rel;
        if at > 0 {
            let prev = lower.as_bytes()[at - 1];
            if !prev.is_ascii_whitespace() && prev != b'<' {
                from = at + 1;
                continue;
            }
        }

        let value_at = at + needle.len();
        let bytes = tag.as_bytes();
        let quote = *bytes.get(value_at)?;
        if quote == b'"' || quote == b'\'' {
            let start = value_at + 1;
            let end = tag[start..].find(quote as char)?;
            return Some(unescape(&tag[start..start + end]));
        }

        let rest = &tag[value_at..];
        let end = rest
            .find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
            .unwrap_or(rest.len());
        let value = rest[..end].trim();
        if value.is_empty() {
            return None;
        }
        return Some(unescape(value));
    }

    None
}

fn unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

#[cfg(test)]
mod tests {
    use slidx_core::{parse_deck, DeckParseOptions};

    use super::*;

    fn slide_of(source: &str) -> Slide {
        parse_deck(source, &DeckParseOptions::default()).slides.remove(0)
    }

    fn deck_of(source: &str) -> slidx_core::Deck {
        parse_deck(source, &DeckParseOptions::default())
    }

    #[test]
    fn a_video_in_markdown_is_still_a_video_in_the_page() {
        // The editor writes a raw HTML tag. If the renderer escaped it, the
        // clip would be a code sample and this feature would measure nothing.
        let html = render_markdown(
            "# Demo\n\n<video src=\"./clip.mp4\" controls></video>\n",
            &MarkdownOptions::default(),
        );

        assert!(html.contains("<video"), "{html}");
        assert_eq!(clips_in(&html)[0].src, "./clip.mp4");
    }

    #[test]
    fn a_slide_with_no_clip_has_no_path_to_a_decoder() {
        assert_eq!(slide_script("<article><p>Hi</p></article>", "./media.js"), "");
    }

    #[test]
    fn a_demo_fallback_is_not_a_clip_the_room_pressed_play_on() {
        // The shell injects this. It is muted on purpose, and measuring it
        // would be a reading about the recording rather than about the talk.
        let frame = "<video class=\"slidx-demo-fallback\" src=\"./checkout.mp4\" muted></video>";
        assert_eq!(slide_script(frame, "./media.js"), "");
        assert!(clips_in(frame).is_empty());
    }

    #[test]
    fn a_clip_on_the_slide_imports_the_controller_and_sets_volume() {
        let emitted = slide_script("<video src=\"./clip.mp4\" controls></video>", "./media.js");

        assert!(emitted.contains("createMediaController"), "{emitted}");
        assert!(emitted.contains("controller.prepare(clip)"), "{emitted}");
        assert!(emitted.contains("controller.release(clip)"), "{emitted}");
        assert!(emitted.contains(r#"from "./media.js""#), "{emitted}");
        assert!(emitted.contains("slidx-demo-fallback"), "{emitted}");
    }

    #[test]
    fn a_fenced_sample_of_a_tag_is_not_a_clip() {
        let html = render_markdown(
            "# HTML\n\n```html\n<video src=\"./nope.mp4\"></video>\n```\n",
            &MarkdownOptions::default(),
        );

        assert!(clips_in(&html).is_empty(), "{html}");
    }

    #[test]
    fn nested_source_elements_are_the_clip_when_the_tag_has_no_src() {
        let html = concat!(
            "<video controls>\n",
            "<source src=\"./a.webm\">\n",
            "<source src=\"./a.mp4\">\n",
            "</video>",
        );

        assert_eq!(
            clips_in(html).iter().map(|clip| clip.src.as_str()).collect::<Vec<_>>(),
            ["./a.webm", "./a.mp4"]
        );
    }

    #[test]
    fn an_escaped_src_is_the_path_the_file_has() {
        assert_eq!(
            clips_in(r#"<audio src="assets/a&amp;b.mp3"></audio>"#)[0].src,
            "assets/a&b.mp3"
        );
    }

    #[test]
    fn the_presenter_names_the_clip_on_the_slide_that_has_not_arrived() {
        let deck =
            deck_of("# One\n\n---\n\n# Two\n\n<video src=\"./loud.mp4\" controls></video>\n");
        let emitted =
            presenter_script(&deck, &deck.slides[0], "./media.js", &MarkdownOptions::default());

        assert!(emitted.contains("measureClip"), "{emitted}");
        assert!(emitted.contains("./loud.mp4"), "{emitted}");
        assert!(emitted.contains(r#"from "./media.js""#), "{emitted}");
        assert!(emitted.contains("const next = "), "{emitted}");
    }

    #[test]
    fn a_presenter_page_with_nothing_coming_imports_nothing() {
        let slide = slide_of("# One\n");
        let deck = deck_of("# One\n");
        assert_eq!(presenter_script(&deck, &slide, "./media.js", &MarkdownOptions::default()), "");
    }

    #[test]
    fn it_imports_from_the_address_the_builder_supplies() {
        let emitted = slide_script("<audio src=\"./a.mp3\"></audio>", "/deck/media.js");
        assert!(emitted.contains(r#"from "/deck/media.js""#), "{emitted}");
    }
}
