//! The print stylesheet.
//!
//! A printed deck is a different artefact from a projected one, and the
//! differences are deliberate rather than incidental:
//!
//! - **One page is one stop.** Page breaks are the only layout that matters.
//! - **No scaling.** The page *is* the slide's size, set by `@page`, so the
//!   container queries resolve against a fixed box and the type lands at the
//!   size it was designed at.
//! - **Ink, not light.** The canvas background is dropped; a dark theme that
//!   printed its background would use a cartridge per handout.

/// The print stylesheet, inlined into the shell.
pub const STYLESHEET: &str = r#"
*, *::before, *::after { box-sizing: border-box; }

html, body {
  margin: 0;
  padding: 0;
  background: var(--slidx-color-canvas);
  color: var(--slidx-color-text);
  font-family: var(--slidx-font-sans);
}

.slidx-print { display: block; }

/*
 * One stop, one page.
 *
 * The size comes from `@page`, so a slide is not scaled to fit anything: the
 * page was made the slide's shape instead.
 */
.slidx-page {
  display: block;
  width: 100%;
  aspect-ratio: var(--slidx-slide-width) / var(--slidx-slide-height);
  break-after: page;
  page-break-after: always;
  overflow: hidden;
}

.slidx-page:last-child { break-after: auto; page-break-after: auto; }

.slidx-slide {
  position: relative;
  width: 100%;
  height: 100%;
  container-type: size;
  display: flex;
  flex-direction: column;
  padding: var(--slidx-space-padding);
  background: var(--slidx-color-surface);
}

.slidx-slide-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: var(--slidx-space-block);
  font-size: var(--slidx-size-body);
  line-height: 1.5;
}

[data-slidx-layout="top"] .slidx-slide-body { justify-content: flex-start; }

.slidx-slide-body > * { margin: 0; }

h1, h2, h3, h4, h5, h6 {
  color: var(--slidx-color-heading);
  max-width: 22ch;
  font-weight: 650;
  line-height: 1.15;
  letter-spacing: -0.015em;
}

h1 { font-size: var(--slidx-size-heading-1); }
h2 { font-size: var(--slidx-size-heading-2); }
h3 { font-size: var(--slidx-size-heading-3); }

strong { color: var(--slidx-color-accent); font-weight: 650; }
a { color: var(--slidx-color-accent); }

ul, ol {
  margin: 0;
  padding-left: 1.4em;
  display: flex;
  flex-direction: column;
  gap: calc(var(--slidx-space-block) * 0.5);
}

blockquote {
  margin: 0;
  padding-left: 0.8em;
  border-left: calc(var(--slidx-hairline) * 3) solid var(--slidx-color-accent);
  color: var(--slidx-color-muted);
}

code { font-family: var(--slidx-font-mono); font-size: var(--slidx-size-code); }

pre {
  margin: 0;
  padding: 0.8em 1em;
  overflow: hidden;
  background: var(--slidx-color-code-surface);
  color: var(--slidx-color-code-text);
  border-radius: var(--slidx-radius);
}

table { border-collapse: collapse; width: 100%; font-size: var(--slidx-size-body); }

th, td {
  padding: 0.4em 0.6em;
  text-align: left;
  border-bottom: var(--slidx-hairline) solid var(--slidx-color-border);
}

th { color: var(--slidx-color-muted); font-weight: 600; }

img { max-width: 100%; height: auto; }

.slidx-slide-footer {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  padding-top: calc(var(--slidx-space-block) * 0.75);
  color: var(--slidx-color-muted);
  font-size: var(--slidx-size-caption);
  font-variant-numeric: tabular-nums;
}

.slidx-accent { color: var(--slidx-color-accent); }
.slidx-muted { color: var(--slidx-color-muted); }
.slidx-code { font-family: var(--slidx-font-mono); }

[data-slidx-step] { display: none; }

/*
 * Hidden means hidden, on paper too.
 *
 * A page for stop 2 that showed stop 5's content would make the whole handout
 * wrong in the one way nobody proofreads for.
 */
[data-slidx-hidden] { visibility: hidden; }

/* Motion has no meaning on paper, and a mid-animation frame prints blurred. */
[data-slidx-effect] {
  animation: none !important;
  transition: none !important;
  transform: none !important;
  opacity: 1 !important;
}

@media print {
  /* Ink, not light: a dark theme's canvas would cost a cartridge a handout. */
  html, body { background: transparent; }
  .slidx-page { break-inside: avoid; }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_stop_is_one_page() {
        assert!(STYLESHEET.contains("page-break-after: always"));
        assert!(STYLESHEET.contains("break-after: page"));
    }

    #[test]
    fn the_last_page_does_not_add_a_blank_one() {
        assert!(STYLESHEET.contains(".slidx-page:last-child"));
    }

    #[test]
    fn hidden_elements_stay_hidden_on_paper() {
        // Otherwise the page for stop 2 shows stop 5's content, and the
        // handout is wrong in the one way nobody proofreads for.
        assert!(STYLESHEET.contains("[data-slidx-hidden] { visibility: hidden; }"));
    }

    #[test]
    fn animation_is_cancelled_rather_than_captured_mid_flight() {
        let effects = &STYLESHEET[STYLESHEET.find("[data-slidx-effect]").unwrap()..];
        assert!(effects[..200].contains("animation: none !important"));
        assert!(effects[..200].contains("opacity: 1 !important"));
    }

    #[test]
    fn the_canvas_background_is_dropped_when_printing() {
        let print = &STYLESHEET[STYLESHEET.find("@media print").unwrap()..];
        assert!(print.contains("background: transparent"));
    }

    #[test]
    fn the_slide_is_not_scaled_because_the_page_is_its_size() {
        assert!(!STYLESHEET.contains("transform: scale("));
        assert!(STYLESHEET.contains("container-type: size"));
    }

    #[test]
    fn braces_balance() {
        assert_eq!(STYLESHEET.matches('{').count(), STYLESHEET.matches('}').count());
    }
}
