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
  grid-template-rows: auto auto auto 1fr auto;
  grid-template-areas: "bar bar" "keys keys" "present present" "notes next" "report report";
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
  flex-wrap: wrap;
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

/*
 * Whether the talk will fit, on its own line under the clock.
 *
 * `flex-basis` rather than a second container: the line names the slides it
 * would drop, so it is long, and it belongs to the clock rather than beside it.
 * Empty until there is something true to say — a deck with no declared slot has
 * no pace, only a running number, and a blank space is the honest reading.
 */
.slidx-pace {
  flex-basis: 100%;
  min-height: 1.4em;
  color: var(--slidx-color-muted);
  font-size: 0.95rem;
}

/*
 * The one place colour is used to say something.
 *
 * Behind borrows the clock's overrun colour because it is the same instruction:
 * act now. Ahead does not get one — it is worth knowing and there is nothing to
 * do about it, and a display that coloured every state would colour none.
 */
[data-slidx-status="nearly-done"] .slidx-clock-value { color: #b26a00; }
[data-slidx-status="over"] .slidx-clock-value { color: #b42318; }
[data-slidx-pace-state="behind"] { color: #b42318; }

/*
 * The keys, on the one screen an audience is not looking at.
 *
 * A speaker drives a deck with their hands off the screen, in the dark, so the
 * binding list is something they need to *see* — and a list of keys on the
 * projector is a list the room reads instead of the slide.
 *
 * Two columns, so the keycaps line up down the left and the eye finds the row
 * it wants without reading any of the others.
 */
.slidx-keys {
  grid-area: keys;
  padding: 1rem 1.25rem;
  border: var(--slidx-hairline) solid var(--slidx-color-border);
  border-radius: var(--slidx-radius);
}

.slidx-keys[hidden] { display: none; }

.slidx-keys-list {
  display: grid;
  gap: 0.5rem 1rem;
  margin: 0;
}

.slidx-key {
  display: grid;
  grid-template-columns: minmax(0, 17rem) minmax(0, 1fr);
  align-items: baseline;
  gap: 1rem;
}

.slidx-key dt,
.slidx-key dd { margin: 0; }

/*
 * The alternatives wrap as a group rather than running off the column. Five
 * keys mean the same thing, and a row where the fifth has fallen under the
 * first is still one row a speaker reads in one glance.
 */
.slidx-key dt {
  display: flex;
  flex-wrap: wrap;
  gap: 0.3rem;
}

.slidx-key dd { color: var(--slidx-color-muted); }

/*
 * A keycap without a border radius, because nothing slidx draws has one — the
 * rule is a legibility decision rather than a taste one, and it does not stop
 * applying on the page a speaker reads under stage lighting.
 */
.slidx-keys kbd {
  padding: 0.1rem 0.4rem;
  font: inherit;
  font-size: 0.9rem;
  font-family: var(--slidx-font-mono);
  border: var(--slidx-hairline) solid var(--slidx-color-border);
}

/*
 * What the browser could not do, and where the speaker does it.
 *
 * Under the bar rather than beside it: every item names a menu path, so the
 * lines are long, and this is read once — in the two minutes before a talk —
 * rather than glanced at during one. It collapses again the moment presenting
 * starts, which is why it is a row that can be `hidden` rather than a panel.
 */
.slidx-present {
  grid-area: present;
  display: grid;
  gap: var(--slidx-e-gap, 0.75rem);
  padding: 1rem 1.25rem;
  border: var(--slidx-hairline) solid var(--slidx-color-border);
  border-radius: var(--slidx-radius);
}

.slidx-present[hidden] { display: none; }

.slidx-present-state {
  margin: 0;
  color: var(--slidx-color-muted);
}

/*
 * What moved since last time, under the advice rather than beside it.
 *
 * Muted, because it is the second sentence: the advice is about this
 * rehearsal, and this is about the direction of travel. A speaker reading the
 * report after a run wants the first one first.
 */
.slidx-rehearsal-trend {
  margin: 0;
  color: var(--slidx-color-muted);
}

.slidx-present-checklist {
  display: grid;
  gap: 0.75rem;
  margin: 0;
  padding: 0;
  list-style: none;
}

.slidx-present-checklist li {
  display: grid;
  gap: 0.15rem;
}

/*
 * The setting, then where it lives. "Turn on Do Not Disturb" is useless advice
 * in the two minutes before a talk if you cannot remember which menu it is
 * under, so the path is not the smaller half of the line by accident — it is
 * the half a speaker is actually looking for.
 */
.slidx-present-item { font-weight: 600; }

.slidx-present-where {
  color: var(--slidx-color-muted);
  font-size: 0.95rem;
}

/*
 * A slide that is slipping is worth the one colour this report spends, and only
 * when it is also over budget — which is the pairing `trackRehearsals` already
 * makes for its own sentence. Faster is said in words and not in colour: it is
 * good news, and good news that shouts is news a speaker learns to skip.
 */
[data-slidx-verdict="over"][data-slidx-trend="slower"] { color: #b42318; }

.slidx-presenter-actions {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 0.75rem;
}

.slidx-presenter-actions button,
.slidx-rehearsal-report button {
  padding: 0.5rem 1.1rem;
  font: inherit;
  font-size: 0.95rem;
  color: var(--slidx-color-text);
  background: var(--slidx-color-surface);
  border: var(--slidx-hairline) solid var(--slidx-color-border);
  border-radius: var(--slidx-radius);
  cursor: pointer;
}

.slidx-presenter-actions button:hover,
.slidx-rehearsal-report button:hover { border-color: var(--slidx-color-accent); }

.slidx-presenter-actions button:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

/* A visible focus ring: this page is driven from a clicker or a keyboard. */
.slidx-presenter-actions button:focus-visible,
.slidx-rehearsal-report button:focus-visible {
  outline: 2px solid var(--slidx-color-accent);
  outline-offset: 2px;
}

.slidx-presenter-divider {
  align-self: stretch;
  border-left: var(--slidx-hairline) solid var(--slidx-color-border);
}

.slidx-rehearsal-status {
  color: var(--slidx-color-muted);
  font-size: 0.85rem;
  font-variant-numeric: tabular-nums;
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
  align-self: start;
  display: grid;
  grid-template-rows: auto auto auto;
  align-content: start;
  gap: 0.8rem;
  padding: 1.1rem 1.25rem;
  background: var(--slidx-color-surface);
  border: var(--slidx-hairline) solid var(--slidx-color-border);
  border-radius: var(--slidx-radius);
  overflow-y: auto;
}

.slidx-presenter-next-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 1rem;
}

.slidx-presenter-label {
  margin: 0;
  font-size: 0.8rem;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--slidx-color-muted);
}

.slidx-presenter-next-position {
  color: var(--slidx-color-muted);
  font-size: 0.8rem;
  font-variant-numeric: tabular-nums;
}

/*
 * The iframe gets the slide's aspect ratio and the normal audience renderer
 * sizes its frame to that viewport. It is therefore a real scaled slide, not
 * Markdown reflowed into the presenter's narrow column.
 */
.slidx-presenter-preview {
  display: block;
  width: min(100%, 34rem);
  height: auto;
  margin-inline: auto;
  background: var(--slidx-color-canvas);
  border: var(--slidx-hairline) solid var(--slidx-color-border);
  border-radius: calc(var(--slidx-radius) * 0.7);
  pointer-events: none;
}

.slidx-presenter-cues {
  display: flex;
  flex-wrap: wrap;
  gap: 0.45rem;
  margin: 0;
  padding: 0;
  list-style: none;
}

.slidx-presenter-cues li {
  padding: 0.35rem 0.55rem;
  color: var(--slidx-color-muted);
  background: var(--slidx-color-canvas);
  border: var(--slidx-hairline) solid var(--slidx-color-border);
  border-radius: calc(var(--slidx-radius) * 0.7);
  font-size: 0.78rem;
  line-height: 1.2;
  font-variant-numeric: tabular-nums;
}

.slidx-presenter-cues [data-slidx-cue="optional"] {
  color: var(--slidx-color-text);
  font-weight: 600;
}

.slidx-presenter-empty {
  color: var(--slidx-color-muted);
  font-style: italic;
}

.slidx-rehearsal-report {
  grid-area: report;
  padding: 1.1rem 1.25rem;
  background: var(--slidx-color-surface);
  border: var(--slidx-hairline) solid var(--slidx-color-border);
  border-radius: var(--slidx-radius);
}

.slidx-rehearsal-report-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}

.slidx-rehearsal-report h2,
.slidx-rehearsal-report p {
  margin: 0;
}

.slidx-rehearsal-report .slidx-rehearsal-advice {
  margin-top: 0.8rem;
  font-size: 1.05rem;
  line-height: 1.5;
}

.slidx-rehearsal-report .slidx-rehearsal-total {
  margin-top: 0.45rem;
  color: var(--slidx-color-muted);
  font-variant-numeric: tabular-nums;
}

.slidx-rehearsal-slides {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr));
  gap: 0.5rem 1.25rem;
  margin: 1rem 0 0;
  padding: 0;
  list-style-position: inside;
}

.slidx-rehearsal-slides li {
  padding-top: 0.5rem;
  border-top: var(--slidx-hairline) solid var(--slidx-color-border);
  font-variant-numeric: tabular-nums;
}

[data-slidx-step] { display: none; }

/* One column on a small screen: a phone is a legitimate presenter remote. */
@media (max-width: 720px) {
  .slidx-presenter {
    grid-template-columns: 1fr;
    grid-template-rows: auto auto auto auto;
    grid-template-areas: "bar" "next" "notes" "report";
    align-content: start;
  }

  .slidx-presenter-divider { display: none; }

  .slidx-presenter-preview { width: 100%; }
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
    fn the_rehearsal_report_has_its_own_grid_area() {
        assert!(STYLESHEET.contains(
            r#"grid-template-areas: "bar bar" "keys keys" "present present" "notes next" "report report""#
        ));
        assert!(STYLESHEET.contains("grid-area: report"));
    }

    #[test]
    fn it_collapses_to_one_column_on_a_phone() {
        assert!(STYLESHEET.contains("@media (max-width: 720px)"));
        assert!(STYLESHEET.contains("grid-template-rows: auto auto auto auto"));
    }

    #[test]
    fn braces_balance() {
        assert_eq!(STYLESHEET.matches('{').count(), STYLESHEET.matches('}').count());
    }
}
