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
 * The body is the layout's grid.
 *
 * Which regions it has and where they sit comes from the theme, emitted next to
 * this stylesheet by `slidx_theme::layout`. What is here is the part no layout
 * may change: the body fills the slide inside its padding, and it is a grid, so
 * a region is a `grid-area` and every track is a share of the slide.
 */
.slidx-slide-body {
  position: relative;
  flex: 1;
  min-height: 0;
  display: grid;
  gap: var(--slidx-space-block);
  font-size: var(--slidx-size-body);
  line-height: 1.5;
}

/*
 * Content is centred in its region by default.
 *
 * A slide is a frame, not a page: text pinned to the top with the rest of the
 * frame empty reads as unfinished, and the eye has further to travel from the
 * heading to the first point. A region that wants the top edge — a title band,
 * or `layout: top` — declares it, and the generated rule says so.
 *
 * `min-width: 0` is load-bearing on a grid: without it a long line or a code
 * block sets the column's minimum to its own intrinsic width, and a two-column
 * layout silently stops being two equal columns.
 */
.slidx-region {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: var(--slidx-space-block);
}

/*
 * A block is a box, because an editor has to be able to measure one.
 *
 * The overlay that highlights a block, snaps it to a region boundary, and warns
 * that its type will be too small in a narrower column all read a rectangle. A
 * wrapper with `display: contents` would carry the index and measure as zero.
 */
.slidx-block {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: calc(var(--slidx-space-block) * 0.5);
}

.slidx-block > * { margin: 0; }

/*
 * Directly positioned blocks still inherit every theme token.
 *
 * Coordinates are shares of the safe body, not viewport pixels, so projector,
 * browser, image and PDF output keep one answer. A block enters this mode only
 * when its Markdown-managed style says at least one geometric property.
 */
.slidx-block[data-slidx-freeform] {
  position: absolute;
  left: var(--slidx-element-x, 0%);
  top: var(--slidx-element-y, 0%);
  width: var(--slidx-element-width, max-content);
  height: var(--slidx-element-height, auto);
  max-width: 100%;
}

.slidx-block[data-slidx-element-color],
.slidx-block[data-slidx-element-color] :is(h1, h2, h3, h4, h5, h6) {
  color: var(--slidx-element-color);
}

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

/*
 * Syntax highlighting, decided at build time.
 *
 * Six classes and nothing else — no italics, no weight changes. A projector
 * loses a slanted stem before it loses a hue, and code that mixes weights sets
 * a different amount of ink per line, which reads as ragged from row fifteen.
 * The colours are the theme's, and every one of them is held to the same
 * contrast rules as body text.
 */
.slidx-code-comment { color: var(--slidx-color-code-comment); }
.slidx-code-string { color: var(--slidx-color-code-string); }
.slidx-code-number { color: var(--slidx-color-code-number); }
.slidx-code-keyword { color: var(--slidx-color-code-keyword); }
.slidx-code-type { color: var(--slidx-color-code-type); }
.slidx-code-punctuation { color: var(--slidx-color-code-punctuation); }

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

/*
 * The speaker, on the slide.
 *
 * The tile occupies the region the author named, so it is a share of the slide
 * like everything else and stays put across every projector. It is a sibling of
 * the regions rather than a child of one, which is what lets the speaker sit
 * over a diagram — the placement a talk actually wants when the region already
 * has something in it.
 *
 * `idle` is the state the build emits and the only state a published page ever
 * has, because nothing outside presentation mode writes the attribute. Drawn as
 * nothing at all: an empty rectangle on every audience slide would be worse
 * than the feature not existing.
 */
.slidx-camera {
  margin: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  border-radius: var(--slidx-radius);
  background: var(--slidx-color-code-surface);
}

[data-slidx-camera-state="idle"] { display: none; }

/*
 * `cover` rather than `contain`: a camera's aspect ratio has nothing to do with
 * the region's, and letterboxing a face inside a tile that is already small is
 * how the speaker ends up as a stripe. Not mirrored — this is the picture the
 * audience sees, and a mirrored one reverses anything held up to it.
 */
.slidx-camera video {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

/*
 * What happened, when nothing happened.
 *
 * A refused permission, a camera another application is holding, a laptop with
 * no camera at all: each leaves the tile saying so rather than showing a black
 * rectangle the speaker has to interpret from the stage.
 */
.slidx-camera-status {
  padding: 0.5em;
  text-align: center;
  color: var(--slidx-color-muted);
  font-size: var(--slidx-size-caption);
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
    fn every_token_the_highlighter_emits_has_a_rule_to_colour_it() {
        // The seam between two crates that never see each other: the scanner
        // writes a class, the theme defines a property, and this is the only
        // place they meet. A missing rule renders that token in the inherited
        // colour, which looks like the scanner failed to recognise it.
        for token in slidx_highlight::Token::COLOURED {
            let class = token.class().unwrap();
            assert!(STYLESHEET.contains(&format!(".{class} {{")), "no rule for {class}");
            assert!(
                STYLESHEET.contains(&format!("var(--slidx-color-code-{})", token.as_token())),
                "{class} does not read the theme's colour"
            );
        }
    }

    #[test]
    fn highlighting_changes_colour_and_nothing_else() {
        // A projector loses a slanted stem before it loses a hue, and mixed
        // weights put a different amount of ink on each line, which reads as
        // ragged from the back of a room.
        for token in slidx_highlight::Token::COLOURED {
            let class = token.class().unwrap();
            let at = STYLESHEET.find(&format!(".{class} {{")).unwrap();
            let rest = &STYLESHEET[at..];
            let rule = &rest[..rest.find('}').unwrap()];

            assert_eq!(rule.matches(':').count(), 1, "{class} declares more than a colour");
            assert!(rule.contains("color:"), "{class} declares something other than a colour");
        }
    }

    #[test]
    fn a_camera_that_was_never_started_is_drawn_as_nothing() {
        // The state every published page is in. Painted, it would be an empty
        // rectangle on every audience slide of every deck that declares a
        // camera, whether or not anybody is presenting from it.
        assert!(declarations().contains("[data-slidx-camera-state=\"idle\"] { display: none; }"));
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
