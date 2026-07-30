//! The interactive half: raw mode, a keypress, a redraw.
//!
//! Everything about *what* the picker shows is in [`super::screen`], where it
//! is a pure function and can be tested. What is left here is the part that
//! genuinely has to touch a terminal, and it is deliberately small — because
//! the failure mode of this file is somebody's shell left in raw mode, with no
//! echo and no line editing, which looks exactly like a hung machine.
//!
//! Raw mode, restoring the terminal, and decoding a keypress all live in
//! [`crate::terminal`], which the TUI preview uses too. An escape-sequence
//! decoder in two files is two decoders, and the second one is always the one
//! missing the terminal that sends `\x1bOA` for an up arrow.
//!
//! ## Keys
//!
//! Arrows and Ctrl-N/Ctrl-P to move, Enter to choose, Esc or Ctrl-C to give up,
//! anything printable to narrow. The Ctrl pair is there because a picker is
//! used with hands on the home row, and the arrows are there because not
//! everybody knows that.

use std::io::Write;

use super::screen::Screen;
use super::Hit;
use crate::style::Style;
use crate::terminal::{self, RawMode};

/// How the picker ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The index into the list that was handed in.
    Chose(usize),
    Cancelled,
    /// This terminal cannot be driven. The caller prints a list.
    Unavailable,
}

/// Runs the picker until something is chosen or given up on.
pub fn choose(entries: &[Hit<'_>], query: &str, style: &Style) -> Outcome {
    let Some(mut tty) = terminal::open() else {
        return Outcome::Unavailable;
    };

    let Some(_raw) = RawMode::enter() else {
        return Outcome::Unavailable;
    };

    let home = super::user_home();
    let mut state = State::new(entries, query);

    loop {
        let view = state.screen(home.as_deref());
        let frame = view.draw(style);
        let _ = write!(tty, "{frame}");
        let _ = tty.flush();

        let key = act(terminal::read_key(&mut tty));
        // Erased before the next frame, and before returning, so the picker
        // leaves the scrollback as it found it rather than a column of
        // half-finished searches.
        erase(&mut tty, frame.lines().count());

        match key {
            Act::Up => state.up(),
            Act::Down => state.down(),
            Act::Backspace => state.backspace(),
            Act::Narrow(character) => state.push(character),
            Act::Nothing => {}
            Act::Choose => {
                let _ = tty.flush();
                return match state.chosen() {
                    Some(index) => Outcome::Chose(index),
                    None => Outcome::Cancelled,
                };
            }
            Act::Cancel => {
                let _ = tty.flush();
                return Outcome::Cancelled;
            }
        }
    }
}

/// The query and the cursor, and the matches that follow from them.
struct State<'a> {
    all: &'a [Hit<'a>],
    query: String,
    matches: Vec<Hit<'a>>,
    selected: usize,
}

impl<'a> State<'a> {
    fn new(all: &'a [Hit<'a>], query: &str) -> Self {
        let mut state = Self { all, query: query.to_string(), matches: Vec::new(), selected: 0 };
        state.refilter();
        state
    }

    /// Re-ranks, and pulls the cursor back into the list.
    ///
    /// Narrowing a list under a cursor that was further down is the ordinary
    /// case, not an edge one — it happens on nearly every keystroke.
    ///
    /// The match each row carries is recomputed here rather than kept, because
    /// the query changed: a highlight left over from the previous keystroke would
    /// point at the wrong characters, which is worse than none.
    fn refilter(&mut self) {
        self.matches = super::scoring::rank(&self.query, self.all, |hit| hit.entry.haystack())
            .into_iter()
            .map(|(hit, found)| Hit { entry: hit.entry, found })
            .collect();

        self.selected = self.selected.min(self.matches.len().saturating_sub(1));
    }

    fn screen(&'a self, home: Option<&'a std::path::Path>) -> Screen<'a> {
        Screen { query: &self.query, matches: &self.matches, selected: self.selected, home }
    }

    fn up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn down(&mut self) {
        if self.selected + 1 < self.matches.len() {
            self.selected += 1;
        }
    }

    fn push(&mut self, character: char) {
        self.query.push(character);
        self.refilter();
    }

    fn backspace(&mut self) {
        self.query.pop();
        self.refilter();
    }

    /// The position in the list the caller handed in, not in the filtered one.
    fn chosen(&self) -> Option<usize> {
        let picked = self.matches.get(self.selected)?;

        self.all.iter().position(|hit| hit.entry.path == picked.entry.path)
    }
}

/// Moves back over `lines` and clears them, so the next frame draws in place.
///
/// The picker leaves the scrollback as it found it rather than a column of
/// half-finished searches.
fn erase(tty: &mut std::fs::File, lines: usize) {
    for _ in 0..lines {
        // Up one, then clear to the end of the line. Clearing without moving
        // would leave the frame and write the next one under it.
        let _ = write!(tty, "\u{1b}[1A\u{1b}[2K");
    }

    let _ = write!(tty, "\r");
    let _ = tty.flush();
}

/// Maps a keypress onto what the picker does with it.
///
/// A subset of [`terminal::Key`]: the picker has nothing to do with Home, End
/// or the page keys, and a key with no command behind it is inert rather than
/// an error — the same rule `packages/runtime/src/keymap.ts` follows.
fn act(key: terminal::Key) -> Act {
    match key {
        terminal::Key::Up => Act::Up,
        terminal::Key::Down => Act::Down,
        terminal::Key::Enter => Act::Choose,
        terminal::Key::Backspace => Act::Backspace,
        terminal::Key::Escape | terminal::Key::Interrupt => Act::Cancel,
        terminal::Key::Space => Act::Narrow(' '),
        terminal::Key::Char(character) => Act::Narrow(character),
        _ => Act::Nothing,
    }
}

/// What the picker does about one keypress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Act {
    Up,
    Down,
    Choose,
    Backspace,
    Cancel,
    Narrow(char),
    Nothing,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Entry;

    fn deck(path: &str, title: &str) -> Entry {
        let mut entry = Entry::new(path);
        entry.title = Some(title.to_string());
        entry
    }

    fn decks() -> Vec<Entry> {
        vec![
            deck("/talks/vueconf", "Making decks fast"),
            deck("/talks/rustfest", "Ownership for the impatient"),
            deck("/work/arch", "Architecture review"),
        ]
    }

    fn state<'a>(all: &'a [Hit<'a>], query: &str) -> State<'a> {
        State::new(all, query)
    }

    /// What `find::run` hands in: every deck, ranked against the query typed on
    /// the command line, which for these tests is nothing.
    fn refs(entries: &[Entry]) -> Vec<Hit<'_>> {
        entries
            .iter()
            .map(|entry| Hit { entry, found: super::super::scoring::Match::default() })
            .collect()
    }

    #[test]
    fn typing_narrows_the_list() {
        let decks = decks();
        let all = refs(&decks);
        let mut state = state(&all, "");
        assert_eq!(state.matches.len(), 3);

        state.push('v');
        state.push('u');
        state.push('e');

        assert_eq!(state.matches.len(), 1);
        assert_eq!(state.matches[0].entry.path, std::path::PathBuf::from("/talks/vueconf"));
    }

    #[test]
    fn backspace_widens_it_again() {
        let decks = decks();
        let all = refs(&decks);
        let mut state = state(&all, "vueconf");
        assert_eq!(state.matches.len(), 1);

        for _ in 0..7 {
            state.backspace();
        }

        assert_eq!(state.matches.len(), 3);
    }

    #[test]
    fn narrowing_pulls_the_cursor_back_into_the_list() {
        // Happens on nearly every keystroke: the list shrinks under a cursor
        // that was further down. A selection past the end would index out of
        // bounds on the next draw.
        let decks = decks();
        let all = refs(&decks);
        let mut state = state(&all, "");
        state.down();
        state.down();
        assert_eq!(state.selected, 2);

        state.push('v');

        assert!(state.selected < state.matches.len().max(1));
    }

    #[test]
    fn the_cursor_stops_at_both_ends_rather_than_wrapping() {
        // Wrapping from the last row to the first is how somebody chooses the
        // wrong deck: they hold a key down and the selection is not where the
        // motion said it would be.
        let decks = decks();
        let all = refs(&decks);
        let mut state = state(&all, "");

        state.up();
        assert_eq!(state.selected, 0);

        for _ in 0..10 {
            state.down();
        }
        assert_eq!(state.selected, 2);
    }

    #[test]
    fn choosing_reports_the_position_in_the_list_that_was_handed_in() {
        // Not the position in the filtered view, which is what the caller would
        // otherwise index with — and would get the wrong deck.
        let decks = decks();
        let all = refs(&decks);
        let mut state = state(&all, "");
        state.push('a');
        state.push('r');
        state.push('c');
        state.push('h');

        assert_eq!(state.chosen(), Some(2));
    }

    #[test]
    fn choosing_with_nothing_matching_chooses_nothing() {
        let decks = decks();
        let all = refs(&decks);
        let state = state(&all, "zzzzz");

        assert!(state.matches.is_empty());
        assert_eq!(state.chosen(), None);
    }

    #[test]
    fn an_opening_query_is_applied_before_a_key_is_pressed() {
        // `slidx open vue` with more than one match opens the picker already
        // narrowed, rather than making somebody type it twice.
        let decks = decks();
        let all = refs(&decks);

        assert_eq!(state(&all, "talks").matches.len(), 2);
    }

    #[test]
    fn the_keys_the_browser_runtime_uses_move_the_cursor_the_same_way_here() {
        // Muscle memory transfers or it does not. The decoding itself lives in
        // crate::terminal and is tested there; this is the mapping onto what
        // the picker does about it.
        assert_eq!(act(terminal::Key::Up), Act::Up);
        assert_eq!(act(terminal::Key::Down), Act::Down);
    }

    #[test]
    fn enter_chooses_and_both_ways_of_giving_up_cancel() {
        assert_eq!(act(terminal::Key::Enter), Act::Choose);
        assert_eq!(act(terminal::Key::Escape), Act::Cancel);
        assert_eq!(act(terminal::Key::Interrupt), Act::Cancel);
    }

    #[test]
    fn backspace_widens_the_query() {
        assert_eq!(act(terminal::Key::Backspace), Act::Backspace);
    }

    #[test]
    fn a_printable_character_narrows_and_a_key_with_no_command_does_nothing() {
        // A key the picker has nothing to do with is inert rather than an
        // error, which is the rule the runtime's keymap follows too.
        assert_eq!(act(terminal::Key::Char('v')), Act::Narrow('v'));
        assert_eq!(act(terminal::Key::Space), Act::Narrow(' '));
        assert_eq!(act(terminal::Key::Home), Act::Nothing);
        assert_eq!(act(terminal::Key::Ignored), Act::Nothing);
    }
}
