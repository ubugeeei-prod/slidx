//! `slidx tui` — walk a deck in the terminal.
//!
//! # This shows structure and flow. It shows nothing about appearance.
//!
//! That sentence is the module's whole contract, it is printed on every frame,
//! and it is in the help text, because the failure worth designing against is
//! specific: somebody checks a deck here, sees it fit inside the box, walks on
//! stage, and finds that it does not fit the slide. A tool that invited that
//! check would have caused it.
//!
//! A terminal row is not a line of 40pt type. Nothing here knows the font, the
//! theme's scale, the projector, or the back row. What it does know is what the
//! *model* says: how many slides, how many stops, what each stop reveals, how
//! the deck reads end to end, and whether the bullets are eight when you
//! thought they were four.
//!
//! For anything about the room, `slidx lint` is the tool — it models projector
//! washout and angular size at the back row and will tell you what this
//! deliberately will not.
//!
//! ## The model, not the HTML
//!
//! It reads [`slidx_core`]'s parsed deck. Rendering the HTML and stripping tags
//! would be a second renderer, which is the thing this repository refuses to
//! have — and it would imply a fidelity a terminal cannot deliver. Code is the
//! one exception, and it defers to [`slidx_highlight`] rather than deciding for
//! itself what a keyword is.
//!
//! ## The same keys as the deck
//!
//! Navigation matches `packages/runtime/src/keymap.ts`: the arrows, space,
//! PageUp and PageDown, Home and End. Muscle memory either transfers or it does
//! not, and a preview that moved differently from the thing being previewed
//! would be worse than no preview.
//!
//! ## Not a terminal
//!
//! One stop as plain text, then exit. A pipe or a CI job has nobody in it to
//! press a key, and a loop waiting for one there is a hang.

pub mod code;
pub mod outline;
pub mod screen;

use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use slidx_core::{parse_deck, Deck, DeckParseOptions};

use crate::args::Matches;
use crate::lint::source;
use crate::report::{self, INDENT};
use crate::style::{Ink, Style};
use crate::terminal::{self, Key, RawMode};
use crate::{Outcome, OK};

use screen::{Box2, View};

/// What a terminal that will not say its size is assumed to be.
///
/// 80×24 is the size every terminal has been able to do since 1978, and the one
/// a window that will not answer is most likely to actually be.
const FALLBACK: (usize, usize) = (80, 24);

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let separator =
        matches.value("separator").map(str::to_string).unwrap_or_else(|| "---".to_string());

    let path = matches
        .first_positional()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(source::DEFAULT_DIR));

    let deck_source = match source::read(&path, &separator) {
        Ok(found) => found,
        Err(message) => return Outcome::misuse(format!("{message}\n")),
    };

    let deck = parse_deck(
        &deck_source.source,
        &DeckParseOptions { separator, ..DeckParseOptions::default() },
    );

    if deck.slides.is_empty() {
        return Outcome::misuse(format!("{} has no slides in it.\n", deck_source.label));
    }

    let size = Box2::fitting(deck.meta.aspect, columns(), rows());
    let start = starting_slide(matches, &deck);
    let stop = starting_stop(matches, &deck, start);

    // A pipe or a CI job has nobody in it to press a key. One stop, then out.
    if !std::io::stdout().is_terminal() {
        let view = View { deck: &deck, slide: start, stop, size };
        return Outcome::out(screen::frame(&view, style));
    }

    match walk(&deck, Position { slide: start, stop }, size, style) {
        Some(()) => Outcome::default().with_code(OK),
        // No raw mode: an unusual terminal, or Windows without one. Falling
        // back to a single frame keeps the command working rather than failing
        // on a terminal nobody could have predicted.
        None => {
            let view = View { deck: &deck, slide: start, stop, size };
            Outcome::out(screen::frame(&view, style))
        }
    }
}

/// The slide to open on.
fn starting_slide(matches: &Matches, deck: &Deck) -> usize {
    matches
        .value("slide")
        .and_then(|given| given.parse::<usize>().ok())
        // One-based on the command line, because nobody counts slides from
        // zero out loud.
        .map(|number| number.saturating_sub(1))
        .unwrap_or(0)
        .min(deck.slides.len().saturating_sub(1))
}

/// The stop to open on, clamped to what the slide actually has.
fn starting_stop(matches: &Matches, deck: &Deck, slide: usize) -> usize {
    matches
        .value("stop")
        .and_then(|given| given.parse::<usize>().ok())
        .map(|number| number.saturating_sub(1))
        .unwrap_or(0)
        .min(last_stop(deck, slide))
}

/// The interactive loop. `None` when this terminal cannot be driven.
fn walk(deck: &Deck, start: Position, size: Box2, style: &Style) -> Option<()> {
    let mut tty = terminal::open()?;
    let _raw = RawMode::enter()?;

    let mut at = start;
    let mut helping = false;

    loop {
        let view = View { deck, slide: at.slide, stop: at.stop, size };
        let text = if helping { help(style) } else { screen::frame(&view, style) };

        let _ = write!(tty, "{text}");
        let _ = tty.flush();

        let key = terminal::read_key(&mut tty);
        erase(&mut tty, text.lines().count());

        if helping {
            // Any key closes the help, so nobody is trapped in it looking for
            // the way out.
            helping = false;
            if matches!(key, Key::Char('q') | Key::Interrupt) {
                return Some(());
            }
            continue;
        }

        match key {
            Key::Char('q') | Key::Escape | Key::Interrupt => return Some(()),
            Key::Char('?') => helping = true,
            Key::Right | Key::Down | Key::Space | Key::Enter | Key::PageDown => at.forward(deck),
            Key::Left | Key::Up | Key::Backspace | Key::PageUp => at.back(deck),
            Key::Home => at.stop = 0,
            Key::End => at.stop = last_stop(deck, at.slide),
            _ => {}
        }
    }
}

/// Where in the deck the view is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Position {
    slide: usize,
    stop: usize,
}

impl Position {
    /// One press forward: through this slide's stops, then to the next slide.
    ///
    /// The same walk the runtime does. A preview where one press moved a whole
    /// slide would hide exactly the thing somebody opened it to count.
    fn forward(&mut self, deck: &Deck) {
        if self.stop < last_stop(deck, self.slide) {
            self.stop += 1;
        } else if self.slide + 1 < deck.slides.len() {
            self.slide += 1;
            self.stop = 0;
        }
    }

    /// One press back, landing on the *last* stop of the previous slide.
    ///
    /// Going back to a slide's first stop would skip everything it revealed,
    /// which is not where somebody stepping backwards expects to be.
    fn back(&mut self, deck: &Deck) {
        if self.stop > 0 {
            self.stop -= 1;
        } else if self.slide > 0 {
            self.slide -= 1;
            self.stop = last_stop(deck, self.slide);
        }
    }
}

fn last_stop(deck: &Deck, slide: usize) -> usize {
    deck.slides.get(slide).map(|slide| slide.timeline.last_index()).unwrap_or(0)
}

/// The key list, and the sentence about what this is not.
pub fn help(style: &Style) -> String {
    let mut text = format!("{}\n\n", style.paint(Ink::Strong, "slidx tui"));

    for (keys, what) in [
        ("→ ↓ space enter pgdn", "next stop"),
        ("← ↑ backspace pgup", "previous stop"),
        ("home", "first stop on this slide"),
        ("end", "last stop on this slide"),
        ("?", "these keys"),
        ("q esc", "quit"),
    ] {
        text.push_str(&format!("  {}  {what}\n", style.pad(Ink::Strong, keys, 22)));
    }

    text.push('\n');
    text.push_str(&report::flowed(
        "The navigation keys are the ones the deck itself uses, so what you \
         learn here works on stage.",
        INDENT,
        Ink::Faint,
        style,
    ));
    text.push('\n');
    text.push_str(&report::flowed(
        "This view shows structure and flow — how many stops, what each one \
         reveals, how the deck reads. It shows nothing about appearance: not \
         whether text fits, not contrast, not layout. `slidx lint` checks those, \
         and a browser shows them.",
        INDENT,
        Ink::Warn,
        style,
    ));
    text.push_str(&report::flowed("any key to go back", INDENT, Ink::Faint, style));

    text
}

/// Moves back over `lines` and clears them, so the next frame draws in place.
fn erase(tty: &mut std::fs::File, lines: usize) {
    for _ in 0..lines {
        let _ = write!(tty, "\u{1b}[1A\u{1b}[2K");
    }

    let _ = write!(tty, "\r");
    let _ = tty.flush();
}

/// The terminal's width, or the size every terminal has always had.
fn columns() -> usize {
    read_size("COLUMNS").unwrap_or(FALLBACK.0).min(crate::style::WIDTH)
}

fn rows() -> usize {
    // Two rows kept back for the status line and the footer, and one so the
    // frame does not scroll the moment it is drawn.
    read_size("LINES").unwrap_or(FALLBACK.1).saturating_sub(3).max(6)
}

/// A size from the environment.
///
/// `COLUMNS` and `LINES` are what a shell exports and what `stty size` would
/// otherwise be asked for. Not asking is deliberate: a subprocess per redraw is
/// a cost paid on every keypress, and a box that is one column wrong is not a
/// problem worth that.
fn read_size(name: &str) -> Option<usize> {
    std::env::var(name).ok()?.parse::<usize>().ok().filter(|value| *value > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::parse_deck;

    fn deck(source: &str) -> Deck {
        parse_deck(source, &DeckParseOptions::default())
    }

    fn staged() -> Deck {
        deck("---\nsteps:\n  - reveal: \"#b\"\n---\n\n# One\n\n- a\n- [b]{#b}\n\n---\n\n# Two\n")
    }

    fn matches_for(line: &str) -> Matches {
        let argv: Vec<String> = line.split_whitespace().map(String::from).collect();

        match crate::args::parse(&argv) {
            crate::args::Invocation::Run(_, matches) => matches,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    #[test]
    fn one_press_moves_through_a_slides_stops_before_leaving_it() {
        // The same walk the runtime does. A preview where one press moved a
        // whole slide would hide exactly what somebody opened it to count.
        let deck = staged();
        let mut at = Position { slide: 0, stop: 0 };

        at.forward(&deck);
        assert_eq!(at, Position { slide: 0, stop: 1 });

        at.forward(&deck);
        assert_eq!(at, Position { slide: 1, stop: 0 });
    }

    #[test]
    fn going_back_lands_on_the_last_stop_of_the_previous_slide() {
        // Not its first. Going back and skipping everything a slide revealed
        // is not where somebody stepping backwards expects to be.
        let deck = staged();
        let mut at = Position { slide: 1, stop: 0 };

        at.back(&deck);
        assert_eq!(at, Position { slide: 0, stop: 1 });
    }

    #[test]
    fn the_ends_of_the_deck_stop_rather_than_wrapping() {
        // Holding a key down at the end of a deck should not silently return
        // to the beginning of it.
        let deck = staged();

        let mut first = Position { slide: 0, stop: 0 };
        first.back(&deck);
        assert_eq!(first, Position { slide: 0, stop: 0 });

        let mut last = Position { slide: 1, stop: 0 };
        for _ in 0..10 {
            last.forward(&deck);
        }
        assert_eq!(last, Position { slide: 1, stop: 0 });
    }

    #[test]
    fn a_slide_is_named_on_the_command_line_the_way_people_count_them() {
        // One-based, because nobody counts slides from zero out loud.
        let deck = staged();

        assert_eq!(starting_slide(&matches_for("tui --slide 2"), &deck), 1);
        assert_eq!(starting_slide(&matches_for("tui --slide 1"), &deck), 0);
    }

    #[test]
    fn a_slide_number_past_the_end_opens_on_the_last_one() {
        assert_eq!(starting_slide(&matches_for("tui --slide 99"), &staged()), 1);
    }

    #[test]
    fn a_slide_number_that_is_not_a_number_opens_at_the_beginning() {
        assert_eq!(starting_slide(&matches_for("tui --slide first"), &staged()), 0);
    }

    #[test]
    fn a_stop_can_be_named_too_so_a_pipe_can_reach_one_that_is_not_the_first() {
        // Without it, everything but the opening state of every slide is
        // unreachable from a script.
        let deck = staged();

        assert_eq!(starting_stop(&matches_for("tui --stop 2"), &deck, 0), 1);
        assert_eq!(starting_stop(&matches_for("tui"), &deck, 0), 0);
    }

    #[test]
    fn a_stop_past_the_end_of_a_slide_opens_on_its_last_one() {
        assert_eq!(starting_stop(&matches_for("tui --stop 99"), &staged(), 0), 1);
    }

    #[test]
    fn the_help_lists_the_keys_the_deck_itself_uses() {
        // Muscle memory either transfers or it does not. These are the keys in
        // packages/runtime/src/keymap.ts.
        let text = help(&Style::plain());

        for keys in ["→ ↓ space enter pgdn", "← ↑ backspace pgup", "home", "end"] {
            assert!(text.contains(keys), "{keys} is missing from:\n{text}");
        }
    }

    #[test]
    fn the_help_says_what_this_view_cannot_tell_you() {
        // In the help as well as on the frame, because somebody who opened the
        // help is asking what this is, and the honest answer includes the
        // limit.
        let text = help(&Style::plain());

        assert!(text.contains("structure and flow"), "{text}");
        assert!(text.contains("nothing about appearance"), "{text}");
        assert!(text.contains("whether text fits"), "{text}");
        assert!(text.contains("slidx lint"), "{text}");
    }

    #[test]
    fn the_help_says_how_to_leave_it() {
        assert!(help(&Style::plain()).contains("any key to go back"));
    }

    #[test]
    fn a_box_is_never_wider_than_the_reports_fixed_width() {
        // Everything else slidx prints wraps at 80, and a box that did not
        // would be the one thing on screen that does not line up.
        let size = Box2::fitting(slidx_core::AspectRatio::default(), 500, 500);

        assert!(size.width <= 500);
        assert_eq!(columns().min(crate::style::WIDTH), columns());
    }

    #[test]
    fn a_terminal_that_will_not_say_its_size_gets_the_size_every_terminal_has() {
        // 80x24, which is both the oldest answer and the most likely one.
        assert!(columns() > 0);
        assert!(rows() >= 6);
    }
}
