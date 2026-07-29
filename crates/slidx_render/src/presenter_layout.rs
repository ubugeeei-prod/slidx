//! The presenter view's stylesheet.
//!
//! Sized for a laptop screen a speaker glances at, not for a projector. The
//! clock is the largest thing on the page because it is the thing read most
//! often and from the furthest away — a speaker looks up from their notes,
//! reads it in one glance, and looks back down.
//!
//! Colour carries the time status, and only that. Everything else is
//! restrained so the one signal that matters is the one that moves.

/// The presenter stylesheet, inlined into the page.
pub const STYLESHEET: &str = r#"
*, *::before, *::after { box-sizing: border-box; }

html, body {
  margin: 0;
  min-height: 100%;
  background: var(--slidx-color-canvas);
  color: var(--slidx-color-text);
  font-family: var(--slidx-font-sans);
}

.slidx-presenter {
  display: grid;
  grid-template-columns: 1fr minmax(280px, 34%);
  grid-template-rows: auto 1fr;
  grid-template-areas: "bar bar" "notes next";
  gap: 1.5rem;
  padding: 1.5rem;
  min-height: 100vh;
}

.slidx-presenter-bar {
  grid-area: bar;
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 1.5rem;
  padding-bottom: 1.25rem;
  border-bottom: var(--slidx-hairline) solid var(--slidx-color-border);
}

.slidx-clock {
  display: flex;
  align-items: baseline;
  gap: 0.75rem;
}

/*
 * Read from across a lectern, so it is set in the largest size on the page
 * and in tabular figures — digits that change width make a running clock
 * jitter, which draws the eye to the motion rather than to the number.
 */
.slidx-clock-value {
  font-size: clamp(2.5rem, 7vw, 4.5rem);
  font-weight: 600;
  line-height: 1;
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.02em;
}

.slidx-clock-budget {
  color: var(--slidx-color-muted);
  font-size: 1rem;
  font-variant-numeric: tabular-nums;
}

/* The one place colour is used to say something. */
[data-slidx-status="nearly-done"] .slidx-clock-value { color: #b26a00; }
[data-slidx-status="over"] .slidx-clock-value { color: #b42318; }

.slidx-presenter-actions {
  display: flex;
  align-items: baseline;
  gap: 0.75rem;
}

.slidx-presenter-actions button {
  padding: 0.5rem 1.1rem;
  font: inherit;
  font-size: 0.95rem;
  color: var(--slidx-color-text);
  background: var(--slidx-color-surface);
  border: var(--slidx-hairline) solid var(--slidx-color-border);
  border-radius: var(--slidx-radius);
  cursor: pointer;
}

.slidx-presenter-actions button:hover { border-color: var(--slidx-color-accent); }

/* A visible focus ring: this page is driven from a clicker or a keyboard. */
.slidx-presenter-actions button:focus-visible {
  outline: 2px solid var(--slidx-color-accent);
  outline-offset: 2px;
}

.slidx-presenter-position {
  color: var(--slidx-color-muted);
  font-variant-numeric: tabular-nums;
  min-width: 4ch;
  text-align: right;
}

/*
 * Where the speaker is inside a build.
 *
 * Quieter than the slide number, because it changes several times per slide
 * and the slide number is the one a speaker calls out when something goes
 * wrong. Empty on a slide with one stop, and an empty element takes no room.
 */
.slidx-presenter-stop {
  color: var(--slidx-color-muted);
  font-variant-numeric: tabular-nums;
  opacity: 0.7;
}

.slidx-presenter-stop:empty {
  display: none;
}

/*
 * Notes get the room, not the current slide.
 *
 * The speaker can already see the slide — it is on the wall behind them. What
 * they cannot see is what they meant to say about it.
 */
.slidx-presenter-notes {
  grid-area: notes;
  font-size: clamp(1.05rem, 1.6vw, 1.4rem);
  line-height: 1.65;
  overflow-y: auto;
}

.slidx-presenter-notes > * { margin: 0 0 0.9em; }
.slidx-presenter-notes ul, .slidx-presenter-notes ol { padding-left: 1.3em; }

.slidx-presenter-next {
  grid-area: next;
  padding: 1.1rem 1.25rem;
  background: var(--slidx-color-surface);
  border: var(--slidx-hairline) solid var(--slidx-color-border);
  border-radius: var(--slidx-radius);
  overflow: hidden;
}

.slidx-presenter-label {
  margin: 0 0 0.75rem;
  font-size: 0.8rem;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--slidx-color-muted);
}

/*
 * The preview is scaled down rather than reflowed, so it reads as a smaller
 * copy of the next slide instead of as a different slide.
 */
.slidx-presenter-preview {
  font-size: 0.72rem;
  line-height: 1.4;
  color: var(--slidx-color-muted);
}

.slidx-presenter-preview h1,
.slidx-presenter-preview h2,
.slidx-presenter-preview h3 {
  font-size: 1.5em;
  margin: 0 0 0.4em;
  color: var(--slidx-color-text);
}

.slidx-presenter-preview ul { padding-left: 1.2em; margin: 0; }
.slidx-presenter-preview pre { display: none; }

.slidx-presenter-empty {
  color: var(--slidx-color-muted);
  font-style: italic;
}

[data-slidx-step] { display: none; }

/* One column on a small screen: a phone is a legitimate presenter remote. */
@media (max-width: 720px) {
  .slidx-presenter {
    grid-template-columns: 1fr;
    grid-template-areas: "bar" "next" "notes";
  }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_is_the_largest_thing_on_the_page() {
        // It is read from the furthest away and most often.
        assert!(STYLESHEET.contains("font-size: clamp(2.5rem, 7vw, 4.5rem)"));
    }

    #[test]
    fn the_clock_uses_tabular_figures() {
        // Digits that change width make a running clock jitter.
        let clock = &STYLESHEET[STYLESHEET.find(".slidx-clock-value").unwrap()..];
        assert!(clock[..300].contains("font-variant-numeric: tabular-nums"));
    }

    #[test]
    fn colour_is_reserved_for_the_time_status() {
        assert!(STYLESHEET.contains(r#"[data-slidx-status="over"]"#));
        assert!(STYLESHEET.contains(r#"[data-slidx-status="nearly-done"]"#));
    }

    #[test]
    fn the_controls_show_a_focus_ring() {
        // Driven from a clicker or a keyboard, where there is no pointer.
        assert!(STYLESHEET.contains("button:focus-visible"));
    }

    #[test]
    fn it_collapses_to_one_column_on_a_phone() {
        assert!(STYLESHEET.contains("@media (max-width: 720px)"));
    }

    #[test]
    fn braces_balance() {
        assert_eq!(STYLESHEET.matches('{').count(), STYLESHEET.matches('}').count());
    }
}
