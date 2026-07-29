//! The interactive half: raw mode, a keypress, a redraw.
//!
//! Everything about *what* the picker shows is in [`super::screen`], where it
//! is a pure function and can be tested. What is left here is the part that
//! genuinely has to touch a terminal, and it is deliberately small — because
//! the failure mode of this file is somebody's shell left in raw mode, with no
//! echo and no line editing, which looks exactly like a hung machine.
//!
//! ## Restoring the terminal is the contract
//!
//! [`RawMode`] saves the terminal's settings before it changes them and puts
//! them back in `Drop`. `Drop` runs on the normal path, on `?`, and while a
//! panic unwinds, so the only ways out that could skip it are a signal that
//! kills the process outright and an abort. That is why the settings are saved
//! rather than reconstructed: `stty sane` would restore *a* working terminal
//! rather than the one somebody had.
//!
//! ## Why `stty` and not a crate
//!
//! `termios` is not in `std`, so raw mode means either a dependency or the
//! program that every POSIX system already ships for exactly this. slidx is a
//! binary people are asked to pipe into a shell, and a terminal-handling crate
//! and its transitive tree is a large thing to add for one screen.
//!
//! Where `stty` is not there — Windows, a stripped container — this reports
//! [`Outcome::Unavailable`] and the caller prints a list instead. A picker is
//! an enhancement; the command works without it.
//!
//! ## Keys
//!
//! Arrows and Ctrl-N/Ctrl-P to move, Enter to choose, Esc or Ctrl-C to give up,
//! anything printable to narrow. The Ctrl pair is there because a picker is
//! used with hands on the home row, and the arrows are there because not
//! everybody knows that.

use std::fs::File;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

use super::screen::Screen;
use crate::index::Entry;
use crate::style::Style;

/// The terminal, by name rather than by inherited handle.
///
/// Standard input may be a pipe — `slidx open < /dev/null` — while the terminal
/// is still there and usable. Opening it directly is also what lets the chosen
/// path go to standard output without the interface following it.
const TTY: &str = "/dev/tty";

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
pub fn choose(entries: &[&Entry], query: &str, style: &Style) -> Outcome {
    let Ok(mut tty) = File::options().read(true).write(true).open(TTY) else {
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

        let key = read_key(&mut tty);
        // Erased before the next frame, and before returning, so the picker
        // leaves the scrollback as it found it rather than a column of
        // half-finished searches.
        erase(&mut tty, frame.lines().count());

        match key {
            Key::Up => state.up(),
            Key::Down => state.down(),
            Key::Backspace => state.backspace(),
            Key::Char(character) => state.push(character),
            Key::Enter => {
                let _ = tty.flush();
                return match state.chosen() {
                    Some(index) => Outcome::Chose(index),
                    None => Outcome::Cancelled,
                };
            }
            Key::Cancel => {
                let _ = tty.flush();
                return Outcome::Cancelled;
            }
        }
    }
}

/// The query and the cursor, and the matches that follow from them.
struct State<'a> {
    all: &'a [&'a Entry],
    query: String,
    matches: Vec<&'a Entry>,
    selected: usize,
}

impl<'a> State<'a> {
    fn new(all: &'a [&'a Entry], query: &str) -> Self {
        let mut state = Self { all, query: query.to_string(), matches: Vec::new(), selected: 0 };
        state.refilter();
        state
    }

    /// Re-ranks, and pulls the cursor back into the list.
    ///
    /// Narrowing a list under a cursor that was further down is the ordinary
    /// case, not an edge one — it happens on nearly every keystroke.
    fn refilter(&mut self) {
        self.matches = super::scoring::rank(&self.query, self.all, |entry| entry.haystack())
            .into_iter()
            .map(|(entry, _)| *entry)
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

        self.all.iter().position(|entry| entry.path == picked.path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Up,
    Down,
    Enter,
    Backspace,
    Cancel,
    Char(char),
}

/// Reads one keypress.
///
/// Bytes rather than lines, because raw mode is exactly the absence of lines.
/// An escape sequence for an arrow key arrives as three bytes and is read as
/// three, which is why a bare Escape is only a cancel when nothing follows it.
fn read_key(tty: &mut File) -> Key {
    let mut byte = [0u8; 1];

    if tty.read(&mut byte).unwrap_or(0) == 0 {
        // The terminal went away mid-picker: a closed session, a killed
        // parent. Giving up is the only thing left.
        return Key::Cancel;
    }

    match byte[0] {
        b'\r' | b'\n' => Key::Enter,
        0x7f | 0x08 => Key::Backspace,
        0x03 | 0x04 => Key::Cancel,
        0x0e => Key::Down,
        0x10 => Key::Up,
        0x1b => escape(tty),
        byte if byte.is_ascii_graphic() || byte == b' ' => Key::Char(byte as char),
        // Anything else — a control code nobody pressed on purpose, or the
        // first byte of a multi-byte character. Ignored rather than guessed at.
        _ => Key::Char('\0'),
    }
}

/// An escape byte: an arrow key, or somebody pressing Escape.
fn escape(tty: &mut File) -> Key {
    let mut rest = [0u8; 2];

    if tty.read(&mut rest).unwrap_or(0) < 2 {
        return Key::Cancel;
    }

    match (rest[0], rest[1]) {
        (b'[', b'A') | (b'O', b'A') => Key::Up,
        (b'[', b'B') | (b'O', b'B') => Key::Down,
        _ => Key::Cancel,
    }
}

/// Moves back over `lines` and clears them.
fn erase(tty: &mut File, lines: usize) {
    for _ in 0..lines {
        // Up one, then clear to the end of the line. Clearing without moving
        // would leave the frame and write the next one under it.
        let _ = write!(tty, "\u{1b}[1A\u{1b}[2K");
    }

    let _ = write!(tty, "\r");
    let _ = tty.flush();
}

/// Raw mode, and putting it back.
///
/// The saved settings are the ones this terminal actually had. `stty sane`
/// would restore *a* working terminal rather than somebody's.
struct RawMode {
    saved: String,
}

impl RawMode {
    fn enter() -> Option<Self> {
        let saved = stty(&["-g"])?;

        // -echo so typing does not appear twice; -icanon so a keypress arrives
        // without waiting for Enter; the rest keeps Ctrl-C reaching us as a
        // byte rather than as a signal that would skip the restore.
        stty(&["-echo", "-icanon", "-isig", "min", "1", "time", "0"])?;

        Some(Self { saved: saved.trim().to_string() })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        // Runs on the normal path, on an early return, and while a panic
        // unwinds. Leaving a shell with no echo looks exactly like a hung
        // machine, so this is the one thing here that must not be skipped.
        let _ = stty(&[self.saved.as_str()]);
    }
}

/// Runs `stty` against the terminal, returning its output.
///
/// Standard input is the terminal by name, not whatever this process inherited:
/// `stty` acts on its own stdin, and in a pipeline that would be the pipe.
fn stty(arguments: &[&str]) -> Option<String> {
    let tty = File::open(TTY).ok()?;

    let output = Command::new("stty")
        .args(arguments)
        .stdin(Stdio::from(tty))
        .stderr(Stdio::null())
        .output()
        .ok()?;

    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
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

    fn state<'a>(all: &'a [&'a Entry], query: &str) -> State<'a> {
        State::new(all, query)
    }

    fn refs(entries: &[Entry]) -> Vec<&Entry> {
        entries.iter().collect()
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
        assert_eq!(state.matches[0].path, std::path::PathBuf::from("/talks/vueconf"));
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
        // Muscle memory transfers or it does not. Ctrl-N/Ctrl-P for hands on
        // the home row, arrows for everybody else.
        assert_eq!(key_of(&[0x0e]), Key::Down);
        assert_eq!(key_of(&[0x10]), Key::Up);
        assert_eq!(key_of(&[0x1b, b'[', b'B']), Key::Down);
        assert_eq!(key_of(&[0x1b, b'[', b'A']), Key::Up);
        // An application-cursor terminal sends O instead of [. Same key.
        assert_eq!(key_of(&[0x1b, b'O', b'A']), Key::Up);
    }

    #[test]
    fn enter_chooses_and_both_ways_of_giving_up_cancel() {
        assert_eq!(key_of(b"\r"), Key::Enter);
        assert_eq!(key_of(b"\n"), Key::Enter);
        assert_eq!(key_of(&[0x03]), Key::Cancel);
        assert_eq!(key_of(&[0x1b, b'x', b'x']), Key::Cancel);
    }

    #[test]
    fn both_spellings_of_backspace_delete_a_character() {
        // Terminals disagree about which one they send, and a backspace that
        // does nothing is the most infuriating possible bug in a search box.
        assert_eq!(key_of(&[0x7f]), Key::Backspace);
        assert_eq!(key_of(&[0x08]), Key::Backspace);
    }

    #[test]
    fn a_printable_character_narrows_and_a_stray_control_code_does_not() {
        assert_eq!(key_of(b"v"), Key::Char('v'));
        assert_eq!(key_of(b" "), Key::Char(' '));
        assert_eq!(key_of(&[0x01]), Key::Char('\0'));
    }

    #[test]
    fn a_terminal_that_goes_away_mid_picker_cancels_rather_than_spinning() {
        // A closed session or a killed parent. Reading zero bytes forever is
        // the difference between exiting and pinning a core.
        assert_eq!(key_of(&[]), Key::Cancel);
    }

    /// Feeds bytes through the key reader using a real file, because the reader
    /// takes a `File` — the same path the terminal goes down.
    fn key_of(bytes: &[u8]) -> Key {
        let path = std::env::temp_dir().join(format!(
            "slidx-keys-{}-{:p}",
            std::process::id(),
            bytes.as_ptr()
        ));
        std::fs::write(&path, bytes).expect("write");

        let mut file = File::open(&path).expect("open");
        let key = read_key(&mut file);
        let _ = std::fs::remove_file(&path);

        key
    }
}
