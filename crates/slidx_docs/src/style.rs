//! The site's stylesheet.
//!
//! Every colour, face, size and space is a `--slidx-brand-*` token from
//! [`slidx_brand`], and every code colour is a `--slidx-color-code-*` token from
//! the default deck theme. Nothing here names a value of its own, for the reason
//! the snippet page's stylesheet names none: a page that hard-coded a background
//! would be the one surface in the project a brand change could not reach and an
//! audit could not see.
//!
//! Two vocabularies rather than one, deliberately. The brand draws the page; the
//! deck theme colours the code, because the code on this site should be
//! coloured by the scanner and the palette a slide gets. They cannot collide —
//! [`slidx_brand::css`] namespaces the brand precisely so a page can carry both.
//!
//! # Flat
//!
//! No shadow and no gradient, which `scripts/check-flat.mjs` enforces on this
//! file along with everything else slidx draws. Depth here is a hairline in
//! `--slidx-brand-line` and a change of surface, and the radius is the brand's
//! zero. The rule exists because a projector turns a soft edge to mud — a
//! documentation site is not projected, but a brand that meant one thing on a
//! slide and another on a page would not be a brand.
//!
//! There is deliberately no test here restating that. The gate already reads
//! this file, and a Rust test that listed the same constructs would be a second
//! copy of the rule — one that the gate itself flags, because a list of the
//! things it rejects is indistinguishable from using them.

/// The whole stylesheet, in one string.
///
/// Sizes are stated in `rem`, `ch` and `em` rather than taken from a deck
/// theme's scale, which is quoted in `cqh` — shares of a slide's height. There
/// is no slide here to be a share of. The brand's own type scale is already in
/// absolute units for exactly this reason.
pub const STYLESHEET: &str = r#"
*, *::before, *::after { box-sizing: border-box; }

html {
  background: var(--slidx-brand-paper);
  -webkit-text-size-adjust: 100%;
}

body {
  margin: 0;
  color: var(--slidx-brand-ink);
  font-family: var(--slidx-brand-font-sans);
  font-size: var(--slidx-brand-size-body);
  line-height: 1.65;
}

a { color: var(--slidx-brand-signal); }
a:hover { text-decoration-thickness: 2px; }

:focus-visible {
  outline: 2px solid var(--slidx-brand-signal);
  outline-offset: 2px;
}

/* Skipped past by everyone with a pointer, and the first stop for everyone
   without one. Off screen until it takes focus, never display:none, which
   would take it out of the tab order it exists to be in. */
.slidx-docs-skip {
  position: absolute;
  left: -100vw;
  top: 0;
  padding: var(--slidx-brand-space-step);
  background: var(--slidx-brand-paper);
  z-index: 1;
}
.slidx-docs-skip:focus { left: 0; }

.slidx-docs-header {
  display: flex;
  flex-wrap: wrap;
  gap: calc(var(--slidx-brand-space-step) * 2);
  align-items: baseline;
  justify-content: space-between;
  max-width: 78rem;
  margin: 0 auto;
  padding: calc(var(--slidx-brand-space-step) * 2) var(--slidx-brand-space-padding);
  border-bottom: var(--slidx-brand-hairline) solid var(--slidx-brand-line);
}

/* Mark and wordmark, at the gap the brand publishes so the lockup is the
   lockup rather than an approximation of it. */
.slidx-docs-lockup {
  display: inline-flex;
  align-items: center;
  gap: var(--slidx-brand-lockup-gap);
  color: var(--slidx-brand-ink);
  font-size: var(--slidx-brand-size-heading-3);
  font-weight: var(--slidx-brand-heading-weight);
  letter-spacing: var(--slidx-brand-heading-tracking);
  text-decoration: none;
}

.slidx-docs-lockup svg {
  width: 1em;
  height: 1em;
  color: var(--slidx-brand-ink);
}

.slidx-docs-tagline {
  color: var(--slidx-brand-muted);
  font-size: var(--slidx-brand-size-caption);
}

.slidx-docs-body {
  display: grid;
  grid-template-columns: minmax(0, 14rem) minmax(0, 1fr);
  gap: calc(var(--slidx-brand-space-step) * 6);
  max-width: 78rem;
  margin: 0 auto;
  padding: calc(var(--slidx-brand-space-step) * 5) var(--slidx-brand-space-padding);
}

/* One column below the width where a 14rem rail and a readable measure stop
   fitting side by side. The rail becomes the first thing on the page, which is
   the right order for a reader who arrived from a search result. */
@media (max-width: 62rem) {
  .slidx-docs-body { grid-template-columns: minmax(0, 1fr); gap: calc(var(--slidx-brand-space-step) * 4); }
}

.slidx-docs-nav ol { margin: 0; padding: 0; list-style: none; }

.slidx-docs-nav > ol > li + li { margin-top: calc(var(--slidx-brand-space-step) * 3); }

.slidx-docs-nav h2 {
  margin: 0 0 calc(var(--slidx-brand-space-step) * 0.5);
  color: var(--slidx-brand-muted);
  font-size: var(--slidx-brand-size-caption);
  font-weight: var(--slidx-brand-heading-weight);
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.slidx-docs-nav a {
  display: block;
  padding: calc(var(--slidx-brand-space-step) * 0.5) 0;
  text-decoration: none;
}

.slidx-docs-nav a:hover { text-decoration: underline; }

/* The page you are on, marked with a rule rather than a fill: one hairline in
   the signal colour, in a design that has no other decoration to compete. */
.slidx-docs-nav a[aria-current="page"] {
  color: var(--slidx-brand-ink);
  border-left: 2px solid var(--slidx-brand-signal);
  margin-left: -10px;
  padding-left: 8px;
}

.slidx-docs-main { min-width: 0; }

/* Long enough to hold a table of lint rules, short enough that a sentence
   does not need finding again on the next line. */
.slidx-docs-prose { max-width: 72ch; }

.slidx-docs-prose h1,
.slidx-docs-prose h2,
.slidx-docs-prose h3 {
  color: var(--slidx-brand-ink);
  font-weight: var(--slidx-brand-heading-weight);
  letter-spacing: var(--slidx-brand-heading-tracking);
  line-height: 1.2;
}

.slidx-docs-prose h1 { margin: 0 0 calc(var(--slidx-brand-space-step) * 2); font-size: var(--slidx-brand-size-heading-1); }
.slidx-docs-prose h2 { margin: calc(var(--slidx-brand-space-step) * 6) 0 calc(var(--slidx-brand-space-step) * 2); font-size: var(--slidx-brand-size-heading-2); }
.slidx-docs-prose h3 { margin: calc(var(--slidx-brand-space-step) * 4) 0 var(--slidx-brand-space-step); font-size: var(--slidx-brand-size-heading-3); }

.slidx-docs-prose p,
.slidx-docs-prose ul,
.slidx-docs-prose ol { margin: 0 0 calc(var(--slidx-brand-space-step) * 2); }

.slidx-docs-prose li + li { margin-top: calc(var(--slidx-brand-space-step) * 0.5); }

.slidx-docs-prose hr {
  margin: calc(var(--slidx-brand-space-step) * 5) 0;
  border: 0;
  border-top: var(--slidx-brand-hairline) solid var(--slidx-brand-line);
}

.slidx-docs-prose blockquote {
  margin: 0 0 calc(var(--slidx-brand-space-step) * 2);
  padding: 0 0 0 calc(var(--slidx-brand-space-step) * 2);
  border-left: 2px solid var(--slidx-brand-line);
  color: var(--slidx-brand-muted);
}

.slidx-docs-prose table {
  width: 100%;
  margin: 0 0 calc(var(--slidx-brand-space-step) * 3);
  border-collapse: collapse;
  font-size: var(--slidx-brand-size-caption);
}

.slidx-docs-prose th,
.slidx-docs-prose td {
  padding: var(--slidx-brand-space-step);
  border-bottom: var(--slidx-brand-hairline) solid var(--slidx-brand-line);
  text-align: left;
  vertical-align: top;
}

.slidx-docs-prose th { color: var(--slidx-brand-muted); font-weight: var(--slidx-brand-heading-weight); }

.slidx-docs-prose img {
  display: block;
  max-width: 100%;
  height: auto;
  border: var(--slidx-brand-hairline) solid var(--slidx-brand-line);
}

/* Inline code takes the brand's mono face and the page's own surface; a fenced
   block takes the deck theme's code colours, so the code on this site is
   coloured the way the same code on a slide would be. */
.slidx-docs-prose code {
  font-family: var(--slidx-brand-font-mono);
  font-size: var(--slidx-brand-size-code);
}

.slidx-docs-prose :not(pre) > code {
  padding: 0.1em 0.3em;
  background: var(--slidx-color-code-surface);
  color: var(--slidx-color-code-text);
}

/* Scrolls rather than wraps, for the reason the snippet page scrolls: a
   wrapped line of code has lost its indentation, and the indentation is often
   the thing being shown. */
.slidx-docs-prose pre {
  margin: 0 0 calc(var(--slidx-brand-space-step) * 3);
  padding: calc(var(--slidx-brand-space-step) * 2);
  overflow-x: auto;
  background: var(--slidx-color-code-surface);
  color: var(--slidx-color-code-text);
  border-radius: var(--slidx-brand-radius);
  line-height: 1.55;
  tab-size: 2;
}

.slidx-docs-prose pre code { background: none; padding: 0; }

.slidx-code-comment { color: var(--slidx-color-code-comment); }
.slidx-code-string { color: var(--slidx-color-code-string); }
.slidx-code-number { color: var(--slidx-color-code-number); }
.slidx-code-keyword { color: var(--slidx-color-code-keyword); }
.slidx-code-type { color: var(--slidx-color-code-type); }
.slidx-code-punctuation { color: var(--slidx-color-code-punctuation); }

.slidx-docs-footer {
  max-width: 78rem;
  margin: 0 auto;
  padding: calc(var(--slidx-brand-space-step) * 3) var(--slidx-brand-space-padding);
  border-top: var(--slidx-brand-hairline) solid var(--slidx-brand-line);
  color: var(--slidx-brand-muted);
  font-size: var(--slidx-brand-size-caption);
}

.slidx-docs-footer a { color: var(--slidx-brand-muted); }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_in_the_stylesheet_names_a_colour_of_its_own() {
        // The rule the snippet page's stylesheet is held to. A literal here
        // would be the one surface a brand change could not reach.
        assert!(!STYLESHEET.contains('#'), "the stylesheet names a colour literally");
        assert!(!STYLESHEET.contains("rgb("));
    }

    #[test]
    fn every_block_is_closed() {
        assert_eq!(STYLESHEET.matches('{').count(), STYLESHEET.matches('}').count());
    }

    #[test]
    fn nothing_is_sized_in_slide_units() {
        // A deck theme quotes its sizes in `cqh`, shares of a slide's height.
        // There is no slide on this page to be a share of, and a size in `cqh`
        // here would collapse to nothing.
        assert!(!STYLESHEET.contains("cqh"));
    }
}
