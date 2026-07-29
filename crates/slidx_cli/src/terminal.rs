//! Raw mode, and reading a keypress.
//!
//! Two commands drive a terminal — the deck picker and the TUI preview — and
//! they need the same three things: put the terminal in raw mode, read one key,
//! put it back. Written once here because an escape-sequence decoder in two
//! files is two decoders, and the second one is always the one missing the
//! terminal that sends `\x1bOA` for an up arrow.
//!
//! ## Restoring the terminal is the contract
//!
//! [`RawMode`] saves the settings before it changes them and puts *those* back
//! in `Drop` — not `stty sane`, which restores a working terminal rather than
//! somebody's. `Drop` runs on the normal path, on an early return, and while a
//! panic unwinds, so the only ways past it are a signal that kills the process
//! outright and an abort. `-isig` keeps Ctrl-C arriving as a byte rather than
//! as a signal, which is what closes the first of those.
//!
//! The failure mode of this file is a shell left with no echo and no line
//! editing, which looks exactly like a hung machine to the person holding it.
//!
//! ## Why `stty` and not a crate
//!
//! `termios` is not in `std`, so raw mode means either a dependency or the
//! program every POSIX system already ships for exactly this. slidx is a binary
//! people are asked to pipe into a shell, and a terminal crate plus its tree is
//! a large thing to add for two screens.
//!
//! Where `stty` is not there — Windows, a stripped container — [`RawMode::enter`]
//! returns `None` and the caller falls back to something non-interactive. Both
//! commands that use this have such a fallback, and neither treats it as an
//! error.

use std::fs::File;
use std::io::Read;
use std::process::{Command, Stdio};

/// The terminal, by name rather than by inherited handle.
///
/// Standard input may be a pipe while the terminal is still there and usable —
/// `slidx open < /dev/null` — and opening it directly is also what lets a
/// command write its answer to standard output without the interface following
/// it there.
pub const TTY: &str = "/dev/tty";

/// A keypress, decoded.
///
/// One set covering what both callers need. A command binds the subset it has
/// something to do with and ignores the rest, which is the same rule
/// `packages/runtime/src/keymap.ts` follows: a key with no command behind it is
/// inert rather than an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Space,
    Backspace,
    Escape,
    /// Ctrl-C or Ctrl-D. Distinct from Escape because a picker treats them the
    /// same and a viewer might not.
    Interrupt,
    Char(char),
    /// A byte nobody pressed on purpose, or the tail of something unrecognised.
    Ignored,
}

/// Opens the terminal for reading and writing.
pub fn open() -> Option<File> {
    File::options().read(true).write(true).open(TTY).ok()
}

/// Reads one keypress.
///
/// Bytes rather than lines, because raw mode is exactly the absence of lines.
/// An arrow arrives as a three-byte escape sequence and is read as three, which
/// is why a bare Escape is only Escape when nothing follows it.
pub fn read_key(tty: &mut File) -> Key {
    let mut byte = [0u8; 1];

    if tty.read(&mut byte).unwrap_or(0) == 0 {
        // The terminal went away mid-loop: a closed session, a killed parent.
        // Reporting an interrupt is the only thing that ends the caller's loop
        // rather than spinning on end-of-file forever.
        return Key::Interrupt;
    }

    match byte[0] {
        b'\r' | b'\n' => Key::Enter,
        b' ' => Key::Space,
        0x7f | 0x08 => Key::Backspace,
        0x03 | 0x04 => Key::Interrupt,
        // Ctrl-N and Ctrl-P, for hands that stay on the home row.
        0x0e => Key::Down,
        0x10 => Key::Up,
        0x1b => escape(tty),
        byte if byte.is_ascii_graphic() => Key::Char(byte as char),
        _ => Key::Ignored,
    }
}

/// An escape byte: a key that sends a sequence, or somebody pressing Escape.
fn escape(tty: &mut File) -> Key {
    let mut rest = [0u8; 2];

    // Fewer than two bytes behind the escape is Escape itself. A terminal that
    // sends the sequence in pieces would be misread here, and none do: the
    // whole sequence arrives in one write.
    if tty.read(&mut rest).unwrap_or(0) < 2 {
        return Key::Escape;
    }

    match (rest[0], rest[1]) {
        // `[` is the ordinary form; `O` is what a terminal in application
        // cursor mode sends for the same key. Both are that key.
        (b'[', b'A') | (b'O', b'A') => Key::Up,
        (b'[', b'B') | (b'O', b'B') => Key::Down,
        (b'[', b'C') | (b'O', b'C') => Key::Right,
        (b'[', b'D') | (b'O', b'D') => Key::Left,
        (b'[', b'H') | (b'O', b'H') => Key::Home,
        (b'[', b'F') | (b'O', b'F') => Key::End,
        // `\x1b[5~`, `\x1b[6~`, `\x1b[1~`, `\x1b[4~`: a digit and a trailing
        // tilde that has to be consumed or it arrives as the next keypress.
        (b'[', digit @ b'0'..=b'9') => numbered(tty, digit),
        _ => Key::Escape,
    }
}

/// The `\x1b[<n>~` family: page up and down, and some terminals' home and end.
fn numbered(tty: &mut File, digit: u8) -> Key {
    let mut tail = [0u8; 1];
    let _ = tty.read(&mut tail);

    match digit {
        b'5' => Key::PageUp,
        b'6' => Key::PageDown,
        b'1' | b'7' => Key::Home,
        b'4' | b'8' => Key::End,
        _ => Key::Ignored,
    }
}

/// Raw mode, and putting it back.
#[derive(Debug)]
pub struct RawMode {
    saved: String,
}

impl RawMode {
    /// Puts the terminal in raw mode, or reports that it cannot be done.
    pub fn enter() -> Option<Self> {
        let saved = stty(&["-g"])?;

        // -echo so typing does not appear twice; -icanon so a keypress arrives
        // without waiting for Enter; -isig so Ctrl-C reaches us as a byte
        // rather than as a signal that would skip the restore below.
        stty(&["-echo", "-icanon", "-isig", "min", "1", "time", "0"])?;

        Some(Self { saved: saved.trim().to_string() })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = stty(&[self.saved.as_str()]);
    }
}

/// Runs `stty` against the terminal, returning its output.
///
/// Standard input is the terminal by name, not whatever this process
/// inherited: `stty` acts on its own stdin, and in a pipeline that would be
/// the pipe.
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

    /// Feeds bytes through the decoder using a real file, because that is the
    /// same path the terminal goes down.
    fn key_of(bytes: &[u8]) -> Key {
        let path = std::env::temp_dir().join(format!(
            "slidx-key-{}-{:p}",
            std::process::id(),
            bytes.as_ptr()
        ));
        std::fs::write(&path, bytes).expect("write");

        let mut file = File::open(&path).expect("open");
        let key = read_key(&mut file);
        let _ = std::fs::remove_file(&path);

        key
    }

    #[test]
    fn the_arrow_keys_a_presentation_remote_sends_are_decoded() {
        // A remote sends arrows and page keys, never letters. These are the
        // ones that have to work in a dark room.
        assert_eq!(key_of(&[0x1b, b'[', b'A']), Key::Up);
        assert_eq!(key_of(&[0x1b, b'[', b'B']), Key::Down);
        assert_eq!(key_of(&[0x1b, b'[', b'C']), Key::Right);
        assert_eq!(key_of(&[0x1b, b'[', b'D']), Key::Left);
    }

    #[test]
    fn an_application_cursor_terminal_sends_the_same_keys_a_different_way() {
        // tmux and a handful of terminals send `O` instead of `[`. A decoder
        // that knows only one form works on the author's machine and nowhere
        // else.
        assert_eq!(key_of(&[0x1b, b'O', b'A']), Key::Up);
        assert_eq!(key_of(&[0x1b, b'O', b'C']), Key::Right);
    }

    #[test]
    fn page_up_and_down_are_decoded_with_their_trailing_tilde_consumed() {
        // Left unread, the `~` arrives as the next keypress and the deck
        // jumps two stops for one press.
        assert_eq!(key_of(&[0x1b, b'[', b'5', b'~']), Key::PageUp);
        assert_eq!(key_of(&[0x1b, b'[', b'6', b'~']), Key::PageDown);
    }

    #[test]
    fn home_and_end_are_decoded_in_both_spellings_terminals_use() {
        assert_eq!(key_of(&[0x1b, b'[', b'H']), Key::Home);
        assert_eq!(key_of(&[0x1b, b'[', b'F']), Key::End);
        assert_eq!(key_of(&[0x1b, b'[', b'1', b'~']), Key::Home);
        assert_eq!(key_of(&[0x1b, b'[', b'4', b'~']), Key::End);
    }

    #[test]
    fn a_bare_escape_with_nothing_behind_it_is_escape() {
        assert_eq!(key_of(&[0x1b]), Key::Escape);
    }

    #[test]
    fn an_unrecognised_sequence_is_escape_rather_than_a_guess() {
        assert_eq!(key_of(&[0x1b, b'[', b'Z']), Key::Escape);
    }

    #[test]
    fn enter_space_and_backspace_are_decoded_in_every_spelling() {
        assert_eq!(key_of(b"\r"), Key::Enter);
        assert_eq!(key_of(b"\n"), Key::Enter);
        assert_eq!(key_of(b" "), Key::Space);
        // Terminals disagree about which backspace they send, and one that
        // does nothing is the most infuriating bug in any text field.
        assert_eq!(key_of(&[0x7f]), Key::Backspace);
        assert_eq!(key_of(&[0x08]), Key::Backspace);
    }

    #[test]
    fn ctrl_c_arrives_as_a_key_rather_than_as_a_signal() {
        // Which is what `-isig` buys, and what lets Drop put the terminal back
        // instead of the process dying with echo still off.
        assert_eq!(key_of(&[0x03]), Key::Interrupt);
        assert_eq!(key_of(&[0x04]), Key::Interrupt);
    }

    #[test]
    fn the_home_row_pair_moves_the_same_way_the_arrows_do() {
        assert_eq!(key_of(&[0x0e]), Key::Down);
        assert_eq!(key_of(&[0x10]), Key::Up);
    }

    #[test]
    fn a_printable_character_comes_through_and_a_stray_control_code_does_not() {
        assert_eq!(key_of(b"q"), Key::Char('q'));
        assert_eq!(key_of(b"?"), Key::Char('?'));
        assert_eq!(key_of(&[0x01]), Key::Ignored);
    }

    #[test]
    fn a_terminal_that_goes_away_reports_an_interrupt_rather_than_spinning() {
        // Reading zero bytes forever is the difference between exiting and
        // pinning a core.
        assert_eq!(key_of(&[]), Key::Interrupt);
    }
}
