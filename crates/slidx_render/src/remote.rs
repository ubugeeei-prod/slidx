//! The page a phone opens to drive the deck.
//!
//! One document per deck, not per slide. The phone is a third window in
//! the mirror, and a window that navigated on every step would drop the
//! socket and mint a new counter — the failure mirroring already had
//! between two laptops. So this page stays put and sends positions.
//!
//! # What it will not do
//!
//! It will not honour a pairing in the query string. `readPairing` is the
//! reader, and a URL that already leaked into a log is refused rather
//! than replayed. It will not open a socket until a pairing is present
//! and a Worker was named. A deck that never opted in never writes this
//! page.

use slidx_core::Deck;
use slidx_theme::{css, Theme};

/// How to build the phone page.
#[derive(Debug, Clone)]
pub struct RemoteOptions {
    pub theme: Theme,
    /// Module URL of the pairing bundle.
    pub remote_src: String,
}

impl Default for RemoteOptions {
    fn default() -> Self {
        Self { theme: slidx_theme::default_theme(), remote_src: "./remote.js".to_string() }
    }
}

/// Renders the phone remote for a deck.
pub fn render_remote(deck: &Deck, options: &RemoteOptions) -> String {
    let stops: Vec<usize> = deck.slides.iter().map(|slide| slide.timeline.frames().len()).collect();
    let stops_json = serde_json::to_string(&stops).expect("stop counts are JSON");

    format!(
        r#"<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>Remote — {deck_title}</title>
{noindex}
<style>
{theme_css}
{layout_css}
</style>
</head>
<body>
<main class="slidx-remote-page">
  <p class="slidx-remote-status" data-slidx-remote-status>Open the link from the presenter view.</p>
  <p class="slidx-remote-position" data-slidx-remote-position hidden></p>
  <div class="slidx-remote-pad">
    <button type="button" data-slidx-remote-prev disabled>Previous</button>
    <button type="button" data-slidx-remote-next disabled>Next</button>
  </div>
</main>
<script type="module">
{script}
</script>
</body>
</html>
"#,
        lang = escape(deck.language()),
        deck_title = escape(deck.meta.title.as_deref().unwrap_or("slidx")),
        noindex = crate::seo::NOINDEX,
        theme_css = css::render(&options.theme),
        layout_css = STYLESHEET,
        script = script(&options.remote_src, &stops_json),
    )
}

fn script(remote_src: &str, stops_json: &str) -> String {
    format!(
        r#"import {{
  readPairing,
  connectRelay,
  relaySocketUrl,
  joinRemote,
}} from "{remote_src}";

const stops = {stops_json};
const status = document.querySelector("[data-slidx-remote-status]");
const positionLabel = document.querySelector("[data-slidx-remote-position]");
const prev = document.querySelector("[data-slidx-remote-prev]");
const next = document.querySelector("[data-slidx-remote-next]");

const pairing = readPairing(location.href);
const raw = document.documentElement.getAttribute("data-slidx-remote");
let config;
try {{ config = raw ? JSON.parse(raw) : null; }} catch {{ config = null; }}

if (!pairing) {{
  status.textContent = "This link is missing its secret. Open the one from the presenter view.";
}} else if (!config?.endpoint) {{
  status.textContent = "This deck has no relay. The keyboard on the lectern still works.";
}} else {{
  const socket = connectRelay(relaySocketUrl(config.endpoint, pairing.session));
  const mirror = joinRemote({{ pairing, socket, local: false }});

  let slide = 0;
  let step = 0;

  const clamp = () => {{
    if (slide < 0) slide = 0;
    if (slide >= stops.length) slide = Math.max(0, stops.length - 1);
    const last = Math.max(0, (stops[slide] ?? 1) - 1);
    if (step < 0) step = 0;
    if (step > last) step = last;
  }};

  const paint = () => {{
    status.textContent = "Driving the deck.";
    positionLabel.hidden = false;
    positionLabel.textContent = `Slide ${{slide + 1}} of ${{stops.length}}`;
    prev.disabled = slide === 0 && step === 0;
    next.disabled = slide >= stops.length - 1 && step >= (stops[slide] ?? 1) - 1;
  }};

  const send = () => {{
    clamp();
    mirror.send({{ slide, step }});
    paint();
  }};

  const go = (delta) => {{
    if (delta > 0) {{
      if (step + 1 < (stops[slide] ?? 1)) step += 1;
      else if (slide + 1 < stops.length) {{ slide += 1; step = 0; }}
    }} else if (step > 0) {{
      step -= 1;
    }} else if (slide > 0) {{
      slide -= 1;
      step = Math.max(0, (stops[slide] ?? 1) - 1);
    }}
    send();
  }};

  prev.addEventListener("click", () => go(-1));
  next.addEventListener("click", () => go(1));
  addEventListener("keydown", (event) => {{
    if (event.key === "ArrowRight" || event.key === "PageDown" || event.key === " ") {{
      event.preventDefault();
      go(1);
    }} else if (event.key === "ArrowLeft" || event.key === "PageUp" || event.key === "Backspace") {{
      event.preventDefault();
      go(-1);
    }}
  }});

  let touchX = null;
  addEventListener("touchstart", (event) => {{
    touchX = event.changedTouches[0]?.clientX ?? null;
  }}, {{ passive: true }});
  addEventListener("touchend", (event) => {{
    const end = event.changedTouches[0]?.clientX;
    if (touchX === null || end === undefined) return;
    const delta = end - touchX;
    touchX = null;
    if (Math.abs(delta) < 40) return;
    go(delta < 0 ? 1 : -1);
  }}, {{ passive: true }});

  mirror.subscribe((position) => {{
    slide = position.slide;
    step = position.step;
    clamp();
    paint();
  }});
  mirror.requestPosition();
  paint();
}}
"#,
        remote_src = remote_src,
        stops_json = stops_json,
    )
}

const STYLESHEET: &str = r#"
html, body {
  margin: 0;
  min-height: 100%;
  background: var(--slidx-color-canvas);
  color: var(--slidx-color-text);
  font-family: var(--slidx-font-sans);
}

.slidx-remote-page {
  display: grid;
  gap: 1.5rem;
  padding: 1.5rem;
  padding-bottom: max(1.5rem, env(safe-area-inset-bottom));
  min-height: 100vh;
  align-content: center;
}

.slidx-remote-status,
.slidx-remote-position {
  margin: 0;
  text-align: center;
}

.slidx-remote-position {
  font-size: 1.5rem;
  font-variant-numeric: tabular-nums;
}

.slidx-remote-pad {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1rem;
}

.slidx-remote-pad button {
  min-height: 6rem;
  font: inherit;
  font-size: 1.25rem;
  color: inherit;
  background: var(--slidx-color-surface);
  border: var(--slidx-hairline) solid var(--slidx-color-border);
}

.slidx-remote-pad button:disabled { opacity: 0.4; }
"#;

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use slidx_core::{parse_deck, DeckParseOptions};

    use super::*;

    fn page(source: &str) -> String {
        let deck = parse_deck(source, &DeckParseOptions::default());
        render_remote(&deck, &RemoteOptions::default())
    }

    #[test]
    fn it_imports_the_pairing_reader_and_nothing_from_a_cdn() {
        let html = page("# One\n\n---\n\n# Two\n");

        assert!(html.contains("readPairing"), "{html}");
        assert!(html.contains("joinRemote"), "{html}");
        assert!(html.contains(r#"from "./remote.js""#), "{html}");
        assert!(!html.contains("http://"), "{html}");
        assert!(!html.contains("https://"), "{html}");
    }

    #[test]
    fn a_query_string_secret_is_not_a_path_it_honours() {
        // The reader is `readPairing`. This page must call it on location.href
        // and not invent a second reader that would accept a leaked URL.
        let html = page("# One\n");

        assert!(html.contains("readPairing(location.href)"), "{html}");
        assert!(html.contains("missing its secret"), "{html}");
    }

    #[test]
    fn it_carries_one_stop_count_per_slide() {
        let html = page("# One\n\n---\n\n- a <!-- step -->\n- b <!-- step -->\n");

        assert!(html.contains("const stops = [1,3]"), "{html}");
    }

    #[test]
    fn it_is_never_indexed() {
        let html = page("# One\n");

        assert!(html.contains("noindex"), "{html}");
    }
}
