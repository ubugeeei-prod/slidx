//! The shell stylesheet.
//!
//! Everything here is expressed against the theme's custom properties, so a
//! theme can change every colour and every size without this file knowing.
//! The two things it owns are the ones a theme must not be able to break:
//!
//! **The slide scales as one piece.** The slide is a size container, so every
//! length inside is a share of the slide rather than a share of the window. It
//! looks the same on a laptop, a projector, and a PDF page; nothing reflows at
//! a different size, and "shrink the text until it fits" is not reachable —
//! the fix for a full slide is to split it.
//!
//! **The safe area is real padding.** Venue projection crops the edges, so the
//! theme's padding is enforced here rather than left to each layout.

/// The complete shell stylesheet, inlined into every slide page.
pub const STYLESHEET: &str = r#"
*, *::before, *::after { box-sizing: border-box; }

html, body {
  margin: 0;
  height: 100%;
  background: var(--slidx-color-canvas);
  color: var(--slidx-color-text);
  font-family: var(--slidx-font-sans);
  -webkit-text-size-adjust: 100%;
}

.slidx-deck {
  display: grid;
  place-items: center;
  min-height: 100%;
  padding: clamp(0px, 2vmin, 32px);
}

/*
 * The design box.
 *
 * The slide takes the largest size that fits the viewport at its aspect ratio,
 * and `container-type: size` makes it the container the theme's `cqh` sizes
 * resolve against. Everything inside is therefore a share of the slide rather
 * than a share of the window, and the whole slide scales as one piece with no
 * transform and no script.
 *
 * A `transform: scale()` would be the obvious approach and is a dead end: CSS
 * `calc()` cannot divide a length by a length, so there is no way to compute
 * the unitless ratio `scale()` needs.
 */
.slidx-slide {
  position: relative;
  aspect-ratio: var(--slidx-slide-width) / var(--slidx-slide-height);
  width: min(
    100%,
    calc((100vh - 4vmin) * var(--slidx-slide-width) / var(--slidx-slide-height))
  );
  container-type: size;

  display: flex;
  flex-direction: column;
  padding: var(--slidx-space-padding);
  background: var(--slidx-color-surface);
  border-radius: var(--slidx-radius);
  overflow: hidden;
}

/*
 * Content is centred by default.
 *
 * A slide is a frame, not a page: text pinned to the top with the rest of the
 * frame empty reads as unfinished, and the eye has further to travel from the
 * heading to the first point. `top` is available for the slides that genuinely
 * want it, such as a long list that would otherwise straddle the centre line.
 */
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
[data-slidx-layout="split"] .slidx-slide-body {
  display: grid;
  grid-template-columns: 1fr 1fr;
  align-content: center;
}

.slidx-slide-body > * { margin: 0; }

h1, h2, h3, h4, h5, h6 {
  color: var(--slidx-color-heading);
  max-width: 22ch;
  font-weight: 650;
  line-height: 1.15;
  letter-spacing: -0.015em;
  text-wrap: balance;
}

h1 { font-size: var(--slidx-size-heading-1); }
h2 { font-size: var(--slidx-size-heading-2); }
h3 { font-size: var(--slidx-size-heading-3); }
h4, h5, h6 { font-size: var(--slidx-size-body); }

p { text-wrap: pretty; }

strong { color: var(--slidx-color-accent); font-weight: 650; }

a {
  color: var(--slidx-color-accent);
  text-decoration-thickness: 0.06em;
  text-underline-offset: 0.18em;
}

ul, ol {
  margin: 0;
  padding-left: 1.4em;
  display: flex;
  flex-direction: column;
  gap: calc(var(--slidx-space-block) * 0.5);
}

li::marker { color: var(--slidx-color-muted); }

blockquote {
  margin: 0;
  padding-left: 0.8em;
  border-left: calc(var(--slidx-hairline) * 3) solid var(--slidx-color-accent);
  color: var(--slidx-color-muted);
}

code {
  font-family: var(--slidx-font-mono);
  font-size: var(--slidx-size-code);
}

pre {
  margin: 0;
  padding: 0.8em 1em;
  overflow: auto;
  background: var(--slidx-color-code-surface);
  color: var(--slidx-color-code-text);
  border-radius: var(--slidx-radius);
}

pre code { font-size: inherit; }

p > code, li > code {
  padding: 0.1em 0.3em;
  background: var(--slidx-color-code-surface);
  color: var(--slidx-color-code-text);
  border-radius: var(--slidx-radius);
}

table {
  border-collapse: collapse;
  width: 100%;
  font-size: var(--slidx-size-body);
}

th, td {
  padding: 0.4em 0.6em;
  text-align: left;
  border-bottom: var(--slidx-hairline) solid var(--slidx-color-border);
}

th { color: var(--slidx-color-muted); font-weight: 600; }

img { max-width: 100%; height: auto; }

hr {
  border: 0;
  border-top: var(--slidx-hairline) solid var(--slidx-color-border);
  width: 100%;
}

/*
 * The footer carries the deck's identity on every slide.
 *
 * Any single slide can be the one that gets photographed and shared, and a
 * screenshot with no attribution is a screenshot nobody can trace back.
 */
.slidx-slide-footer {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  padding-top: calc(var(--slidx-space-block) * 0.75);
  color: var(--slidx-color-muted);
  font-size: var(--slidx-size-caption);
  font-variant-numeric: tabular-nums;
}

/* Marks: the theme decides what a property means, not the compiler. */
[data-slidx-mark] { transition: color 200ms ease-out; }
.slidx-accent { color: var(--slidx-color-accent); }
.slidx-muted { color: var(--slidx-color-muted); }
.slidx-code { font-family: var(--slidx-font-mono); }

[data-slidx-color="danger"] { color: #b42318; }
[data-slidx-color="success"] { color: #067647; }
[data-slidx-weight="bold"] { font-weight: 700; }

/*
 * A QR tile.
 *
 * Sized as a share of the slide so it scales with everything else, and floored
 * so it stays scannable: below about a fifth of the slide's height a projected
 * code is too small to resolve from the back of a room, which is the only
 * place anyone actually points a phone from.
 */
.slidx-qr {
  margin: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.5em;
  align-self: center;
}

.slidx-qr svg {
  width: max(22cqh, 120px);
  height: auto;
  border-radius: var(--slidx-radius);
}

.slidx-qr-caption {
  color: var(--slidx-color-muted);
  font-size: var(--slidx-size-caption);
  font-family: var(--slidx-font-mono);
}

/*
 * A demo and its recording, stacked.
 *
 * Both sides are laid out at the same size and one is hidden, so switching
 * changes nothing about the geometry — the recording appears exactly where the
 * live demo was, with no reflow for the audience to notice. `display: none` is
 * deliberate over `visibility`: a hidden iframe that still lays out is a hidden
 * iframe still running whatever the demo was doing.
 */
.slidx-demo {
  margin: 0;
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
}

.slidx-demo > * {
  flex: 1 1 auto;
  width: 100%;
  border: 0;
  border-radius: var(--slidx-radius);
  background: var(--slidx-color-code-surface);
  object-fit: contain;
}

[data-slidx-demo="live"] > .slidx-demo-fallback,
[data-slidx-demo="fallback"] > .slidx-demo-live {
  display: none;
}

/* Anchors are addresses, never content. */
[data-slidx-step] { display: none; }

@media print {
  html, body { height: auto; background: #fff; }
  .slidx-deck { padding: 0; overflow: visible; }
  .slidx-slide { width: 100%; page-break-after: always; border-radius: 0; }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_colour_comes_from_a_theme_token() {
        // Two exceptions are deliberate and named in the source: the print
        // background, and the semantic colours a mark can ask for by name.
        let allowed = ["#fff", "#b42318", "#067647"];
        let literals: Vec<&str> = STYLESHEET
            .match_indices('#')
            .map(|(at, _)| {
                let rest = &STYLESHEET[at..];
                let end =
                    rest.find(|c: char| !c.is_ascii_hexdigit() && c != '#').unwrap_or(rest.len());
                &rest[..end]
            })
            .filter(|literal| literal.len() > 1)
            .collect();

        for literal in literals {
            assert!(allowed.contains(&literal), "undeclared colour literal {literal}");
        }
    }

    #[test]
    fn the_slide_scales_as_one_piece() {
        // The property that makes "shrink the text to fit" unreachable: the
        // slide is a container, so every size inside is a share of it.
        assert!(STYLESHEET.contains("container-type: size"));
        assert!(STYLESHEET.contains("aspect-ratio: var(--slidx-slide-width)"));
    }

    #[test]
    fn only_one_side_of_a_demo_is_painted_at_a_time() {
        // The switch is one attribute write and nothing else. If the stylesheet
        // stopped hiding the other side, both would show and the "instant"
        // switch would become a layout change the audience watches happen.
        assert!(STYLESHEET.contains("[data-slidx-demo=\"live\"] > .slidx-demo-fallback"));
        assert!(STYLESHEET.contains("[data-slidx-demo=\"fallback\"] > .slidx-demo-live"));
    }

    #[test]
    fn a_hidden_demo_side_stops_rather_than_lurks() {
        // `visibility: hidden` would leave the live iframe running its demo
        // behind the recording, still holding the network it lost.
        let rules = declarations();
        let at = rules.find("[data-slidx-demo=").expect("the demo switch has no rule");
        assert!(rules[at..].starts_with(
            "[data-slidx-demo=\"live\"] > .slidx-demo-fallback,\n[data-slidx-demo=\"fallback\"] > .slidx-demo-live {\n  display: none;\n}"
        ), "got: {}", &rules[at..at + 120]);
    }

    #[test]
    fn nothing_relies_on_a_scale_transform() {
        // `calc()` cannot divide a length by a length, so a computed scale
        // factor silently evaluates to nothing and the slide renders at its
        // design size. Container queries are the working mechanism.
        assert!(!declarations().contains("transform: scale("));
    }

    /// The stylesheet with comments removed.
    ///
    /// A rule and a note about a rule are different things, and a test that
    /// cannot tell them apart fails on its own documentation.
    fn declarations() -> String {
        let mut out = String::with_capacity(STYLESHEET.len());
        let mut rest = STYLESHEET;

        while let Some(open) = rest.find("/*") {
            out.push_str(&rest[..open]);
            match rest[open..].find("*/") {
                Some(close) => rest = &rest[open + close + 2..],
                None => return out,
            }
        }

        out.push_str(rest);
        out
    }

    #[test]
    fn the_safe_area_is_the_themes_padding() {
        assert!(STYLESHEET.contains("padding: var(--slidx-space-padding)"));
    }

    #[test]
    fn step_anchors_never_render() {
        assert!(STYLESHEET.contains("[data-slidx-step] { display: none; }"));
    }

    #[test]
    fn print_removes_the_scaling_so_a_page_is_a_slide() {
        let print = &STYLESHEET[STYLESHEET.find("@media print").unwrap()..];
        assert!(print.contains("page-break-after: always"));
    }

    #[test]
    fn braces_balance() {
        assert_eq!(STYLESHEET.matches('{').count(), STYLESHEET.matches('}').count());
    }
}
