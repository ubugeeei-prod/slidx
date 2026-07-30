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
//!
//! ## On the terminal
//!
//! The whole window, on the alternate screen, redrawn in place and resized with
//! the window. [`canvas`] owns that and — more to the point — gives it back on
//! every way out, including the one where slidx crashes. Read that module before
//! changing anything here that writes to the terminal.

pub mod canvas;
pub mod code;
pub mod outline;
pub mod screen;

use std::path::PathBuf;

use slidx_core::{parse_deck, Deck, DeckParseOptions};

use crate::args::Matches;
use crate::lint::source;
use crate::report::{self, INDENT};
use crate::style::{Ink, Style};
use crate::terminal::Key;
use crate::{Outcome, OK};

use canvas::Session;
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

    let start = starting_slide(matches, &deck);
    let stop = starting_stop(matches, &deck, start);
    let at = Position { slide: start, stop };

    // A pipe or a CI job has nobody in it to press a key. One stop, then out.
    if !crate::terminal::someone_is_there() {
        return Outcome::out(piped(&deck, at, style));
    }

    match walk(&deck, at, style) {
        Some(()) => Outcome::default().with_code(OK),
        // No raw mode: an unusual terminal, or Windows without one. Falling
        // back to a single frame keeps the command working rather than failing
        // on a terminal nobody could have predicted.
        None => Outcome::out(piped(&deck, at, style)),
    }
}

/// One frame, for something that is not a terminal.
///
/// Held to the fixed width every other slidx report uses, rather than to the
/// window: this ends up in a pipe, a log or an issue, and 80 columns is what
/// survives all three. The interactive view is the one that takes the window,
/// because there it *is* the window.
fn piped(deck: &Deck, at: Position, style: &Style) -> String {
    let size = Box2::fitting(deck.meta.aspect, crate::style::WIDTH, FALLBACK.1.saturating_sub(3));
    let view = View { deck, slide: at.slide, stop: at.stop, size };

    screen::frame(&view, style)
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
///
/// The size is read every frame rather than once, which is what makes the view
/// follow a resize — there is no `SIGWINCH` to wake on without a dependency, and
/// asking costs one `stty` per keypress.
fn walk(deck: &Deck, start: Position, style: &Style) -> Option<()> {
    let mut session = Session::open()?;
    let mut at = start;
    let mut helping = false;

    loop {
        let (rows, columns) = session.size().unwrap_or((FALLBACK.1, FALLBACK.0));
        // Two rows for the status line and the hint bar, one so a frame the
        // exact height of the window does not scroll the moment it is drawn.
        let size = Box2::fitting(deck.meta.aspect, columns, rows.saturating_sub(3).max(6));

        let view = View { deck, slide: at.slide, stop: at.stop, size };
        session.paint(&if helping { help(style) } else { screen::frame(&view, style) });

        let key = session.read();

        if helping {
            // Any key closes the help, so nobody is trapped in it looking for
            // the way out.
            helping = false;
            if matches!(key, Key::Char('q') | Key::Interrupt) {
                return Some(());
            }
            continue;
        }

        match act(key) {
            Act::Quit => return Some(()),
            Act::Help => helping = true,
            Act::Forward => at.forward(deck),
            Act::Back => at.back(deck),
            Act::NextSlide => at.next_slide(deck),
            Act::PreviousSlide => at.previous_slide(),
            Act::FirstStop => at.stop = 0,
            Act::LastStop => at.stop = last_stop(deck, at.slide),
            Act::FirstSlide => at = Position { slide: 0, stop: 0 },
            Act::LastSlide => at = Position { slide: deck.slides.len() - 1, stop: 0 },
            Act::Nothing => {}
        }
    }
}

/// What one keypress does.
///
/// Named rather than matched inline so the bindings are a list a test can walk,
/// and so [`help`] cannot list a key that does nothing — the failure that makes
/// a key list worse than none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Forward,
    Back,
    NextSlide,
    PreviousSlide,
    FirstStop,
    LastStop,
    FirstSlide,
    LastSlide,
    Help,
    Quit,
    Nothing,
}

/// The keymap.
///
/// The arrows, space and the page keys are what `packages/runtime/src/keymap.ts`
/// binds, so muscle memory transfers to the stage. The letters are what a hand
/// already on the home row reaches for, and the wheel is the one thing a mouse
/// is better at than a key.
fn act(key: Key) -> Act {
    match key {
        Key::Char('q') | Key::Escape | Key::Interrupt => Act::Quit,
        Key::Char('?') | Key::Char('h') => Act::Help,
        Key::Right | Key::Down | Key::Space | Key::Enter | Key::PageDown => Act::Forward,
        Key::Left | Key::Up | Key::Backspace | Key::PageUp => Act::Back,
        Key::Char('j') | Key::ScrollDown => Act::Forward,
        Key::Char('k') | Key::ScrollUp => Act::Back,
        Key::Char('n') => Act::NextSlide,
        Key::Char('p') => Act::PreviousSlide,
        Key::Char('g') => Act::FirstSlide,
        Key::Char('G') => Act::LastSlide,
        Key::Home => Act::FirstStop,
        Key::End => Act::LastStop,
        // A key with nothing behind it is inert rather than an error, which is
        // the rule the runtime's keymap follows too.
        _ => Act::Nothing,
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

    /// The next slide, whatever this one has left.
    ///
    /// Different from pressing forward, and both are wanted: stepping through
    /// the stops is how a talk is walked, and skipping to the next slide is how
    /// a deck is looked through.
    fn next_slide(&mut self, deck: &Deck) {
        if self.slide + 1 < deck.slides.len() {
            self.slide += 1;
            self.stop = 0;
        }
    }

    /// The previous slide, at its first stop.
    ///
    /// Its first rather than its last, which is where stepping back lands. The
    /// two keys mean different things: one is walking, and this is looking.
    fn previous_slide(&mut self) {
        if self.slide > 0 {
            self.slide -= 1;
        }

        self.stop = 0;
    }
}

fn last_stop(deck: &Deck, slide: usize) -> usize {
    deck.slides.get(slide).map(|slide| slide.timeline.last_index()).unwrap_or(0)
}

/// Every binding, and what it does.
///
/// One list, read by the help page and — for the two that matter most — by the
/// hint bar under every frame. A key list kept beside a keymap is two lists, and
/// the one that drifts is the list, so [`the_keys_all_do_something`] walks this
/// through [`act`] and fails on a key that was renamed or removed.
///
/// ASCII spellings alongside the arrows, because the arrows are drawn as boxes
/// on a console that has no font for them — and this runs on venue laptops.
pub const KEYS: &[(&str, &str, Act)] = &[
    ("right down space enter pgdn j", "next stop", Act::Forward),
    ("left up backspace pgup k", "previous stop", Act::Back),
    ("n", "next slide, whatever this one has left", Act::NextSlide),
    ("p", "previous slide", Act::PreviousSlide),
    ("home", "first stop on this slide", Act::FirstStop),
    ("end", "last stop on this slide", Act::LastStop),
    ("g", "the first slide", Act::FirstSlide),
    ("G", "the last slide", Act::LastSlide),
    ("?", "these keys", Act::Help),
    ("q esc", "quit", Act::Quit),
];

/// The key list, and the sentence about what this is not.
pub fn help(style: &Style) -> String {
    let mut text = format!("{}\n\n", style.paint(Ink::Strong, "slidx tui"));

    let column = KEYS.iter().map(|(keys, _, _)| crate::style::width::of(keys)).max().unwrap_or(0);
    for (keys, what, _) in KEYS {
        text.push_str(&format!("  {}  {what}\n", style.pad(Ink::Strong, keys, column)));
    }

    // Not in the list above because it is not a key, and because a mouse is
    // discovered by trying it rather than by reading about it.
    text.push_str(&format!(
        "  {}  {}\n",
        style.pad(Ink::Strong, "wheel", column),
        "next and previous stop"
    ));

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
        // packages/runtime/src/keymap.ts, and they are named in the spelling a
        // reader uses rather than drawn as arrows — an arrow glyph is a box on
        // a console with no font for it, and this runs on venue laptops.
        let text = help(&Style::plain());

        for keys in ["right", "down", "space", "enter", "pgdn", "backspace", "pgup", "home", "end"]
        {
            assert!(text.contains(keys), "{keys} is missing from:\n{text}");
        }
    }

    #[test]
    fn nothing_the_help_draws_needs_a_font_the_terminal_might_not_have() {
        // A key list rendered as replacement boxes is a key list nobody reads,
        // and the machine it happens on is the venue's rather than the
        // author's.
        for (keys, what, _) in KEYS {
            assert!(keys.is_ascii(), "{keys} is drawn with a glyph a console may not have");
            assert!(what.is_ascii(), "{what} is drawn with a glyph a console may not have");
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
    fn a_piped_frame_keeps_the_fixed_width_every_other_report_has() {
        // It ends up in a pipe, a log or an issue. The interactive view takes
        // the whole window because there it *is* the window; this one has to
        // survive being pasted next to a doctor report.
        let deck = staged();
        let text = piped(&deck, Position { slide: 0, stop: 0 }, &Style::plain());

        for line in text.lines() {
            assert!(
                crate::style::width::of(line) <= crate::style::WIDTH,
                "{} columns: {line}",
                crate::style::width::of(line)
            );
        }
    }

    #[test]
    fn the_box_follows_the_window_it_is_drawn_in() {
        // The view is redrawn from the size every frame, so a resize changes
        // the box rather than being ignored until the next run. Reading the
        // size once was the bug: `COLUMNS` is not exported by any shell, so
        // every window was drawn as though it were 80 by 24.
        let narrow = Box2::fitting(slidx_core::AspectRatio::default(), 80, 40);
        let wide = Box2::fitting(slidx_core::AspectRatio::default(), 160, 40);

        assert!(wide.width > narrow.width, "{wide:?} is no wider than {narrow:?}");
    }

    #[test]
    fn every_key_the_help_lists_does_what_it_says() {
        // The failure a key list has: a binding renamed in the keymap and left
        // in the list, so the one thing somebody read is the one thing that
        // does nothing. Both come from `KEYS`, and this walks it through the
        // keymap to prove the spellings still land.
        for (keys, what, expected) in KEYS {
            for key in keys.split_whitespace().filter_map(spelling) {
                assert_eq!(act(key), *expected, "`{keys}` ({what}) does not {expected:?}");
            }
        }
    }

    #[test]
    fn the_help_lists_every_binding_the_keymap_has() {
        let text = help(&Style::plain());

        for (keys, what, _) in KEYS {
            assert!(text.contains(keys), "{keys} is bound and not listed:\n{text}");
            assert!(text.contains(what), "{keys} is listed without saying what it does");
        }
    }

    #[test]
    fn a_key_with_nothing_behind_it_is_inert_rather_than_an_error() {
        // The rule `packages/runtime/src/keymap.ts` follows. A deck driven by a
        // clicker gets keys nobody chose to send.
        assert_eq!(act(Key::Char('z')), Act::Nothing);
        assert_eq!(act(Key::Ignored), Act::Nothing);
    }

    #[test]
    fn the_wheel_steps_the_deck_because_that_is_what_a_hand_reaches_for() {
        assert_eq!(act(Key::ScrollDown), Act::Forward);
        assert_eq!(act(Key::ScrollUp), Act::Back);
    }

    #[test]
    fn the_next_slide_key_skips_the_stops_this_one_has_left() {
        // Different from pressing forward, and both are wanted: forward walks a
        // talk, and this looks through a deck.
        let deck = staged();
        let mut at = Position { slide: 0, stop: 0 };

        at.next_slide(&deck);
        assert_eq!(at, Position { slide: 1, stop: 0 });
    }

    #[test]
    fn the_ends_of_the_deck_stop_the_slide_keys_too() {
        let deck = staged();
        let mut first = Position { slide: 0, stop: 0 };
        first.previous_slide();
        assert_eq!(first, Position { slide: 0, stop: 0 });

        let mut last = Position { slide: 1, stop: 0 };
        last.next_slide(&deck);
        assert_eq!(last, Position { slide: 1, stop: 0 });
    }

    /// The key a word in [`KEYS`] names.
    ///
    /// `None` for a spelling that is a name rather than a character — the list
    /// is written for a reader, and `pgdn` is what a reader calls that key.
    fn spelling(word: &str) -> Option<Key> {
        Some(match word {
            "right" => Key::Right,
            "left" => Key::Left,
            "up" => Key::Up,
            "down" => Key::Down,
            "space" => Key::Space,
            "enter" => Key::Enter,
            "backspace" => Key::Backspace,
            "pgdn" => Key::PageDown,
            "pgup" => Key::PageUp,
            "home" => Key::Home,
            "end" => Key::End,
            "esc" => Key::Escape,
            letter if letter.chars().count() == 1 => {
                Key::Char(letter.chars().next().expect("one character"))
            }
            _ => return None,
        })
    }
}
