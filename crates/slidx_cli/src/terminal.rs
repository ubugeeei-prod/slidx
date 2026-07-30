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
use std::io::{IsTerminal, Read};
use std::process::{Command, Stdio};
use std::sync::Mutex;

/// The terminal, by name rather than by inherited handle.
///
/// Standard input may be a pipe while the terminal is still there and usable —
/// `slidx open < /dev/null` — and opening it directly is also what lets a
/// command write its answer to standard output without the interface following
/// it there.
pub const TTY: &str = "/dev/tty";

/// Set by the shell integration while it is holding a command's output.
///
/// [`crate::shell::integration`]'s wrapper function captures standard output so
/// it can follow a path a command printed. That makes stdout a pipe while a
/// person is still sitting in front of the terminal — and every command that
/// asks "is anybody there" would answer no, so the picker would print a list
/// with nobody to choose from it. This is the wrapper saying it is the pipe.
///
/// Read here rather than in each command, because the variable and the script
/// that sets it have to be one spelling.
pub const HELD_BY_A_SHELL_FUNCTION: &str = "SLIDX_SHELL_INTEGRATION";

/// Whether there is a person here to press a key.
pub fn someone_is_there() -> bool {
    watching(
        std::io::stdout().is_terminal(),
        std::env::var(HELD_BY_A_SHELL_FUNCTION).ok().as_deref(),
    )
}

/// The decision, as a function of the two things it depends on.
///
/// Separated from [`someone_is_there`] so both branches are reachable from a
/// test without setting a process-wide variable two parallel tests would fight
/// over — the same shape [`crate::style::wants_color`] uses.
pub fn watching(is_terminal: bool, held: Option<&str>) -> bool {
    is_terminal || held.is_some_and(|value| !value.is_empty())
}

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
    /// A wheel notch. Reported only while a caller has asked for mouse
    /// reporting; without that a terminal sends nothing and these never arrive.
    ScrollUp,
    ScrollDown,
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
        // `\x1b[<64;12;3M` — a mouse report in SGR encoding, which is the only
        // encoding worth decoding: the older one packs coordinates into single
        // bytes and cannot describe a window wider than 223 columns.
        (b'[', b'<') => mouse(tty),
        _ => Key::Escape,
    }
}

/// An SGR mouse report: `<button>;<column>;<row>` and an `M` or an `m`.
///
/// Only the wheel is decoded. A click would have to mean something, and in a
/// view that is one box and a status line there is nothing to click on that a
/// key does not already do — while a wheel is what hands reach for without
/// being told.
///
/// The whole report is consumed either way. Bytes left behind arrive as the
/// next keypress, and a stray `M` would read as a character somebody typed.
fn mouse(tty: &mut File) -> Key {
    let mut button = String::new();
    let mut still_the_button = true;
    let mut byte = [0u8; 1];

    loop {
        if tty.read(&mut byte).unwrap_or(0) == 0 {
            return Key::Ignored;
        }

        match byte[0] {
            b'M' | b'm' => break,
            digit @ b'0'..=b'9' if still_the_button => button.push(digit as char),
            // The first `;`, and everything after it: the column and the row,
            // which nothing here uses and which run to three digits on a large
            // screen.
            _ => still_the_button = false,
        }
    }

    match button.parse::<u8>() {
        Ok(64) => Key::ScrollUp,
        Ok(65) => Key::ScrollDown,
        _ => Key::Ignored,
    }
}

/// The terminal's size in cells, as rows and columns.
///
/// Asked of the terminal rather than of the environment. `COLUMNS` and `LINES`
/// are set by an interactive shell and **not exported**, so a child process
/// reading them almost always sees nothing and falls back — which is how a view
/// ends up drawn at 80×24 inside a window that is neither.
///
/// One subprocess per call, which is the trade: a fork is a millisecond and it
/// happens once per keypress, and the alternative is a view that ignores the
/// window it is in.
pub fn size() -> Option<(usize, usize)> {
    let reported = stty(&["size"])?;
    let mut numbers = reported.split_whitespace().filter_map(|word| word.parse::<usize>().ok());

    match (numbers.next(), numbers.next()) {
        (Some(rows), Some(columns)) if rows > 0 && columns > 0 => Some((rows, columns)),
        _ => None,
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

/// What the terminal was set to before slidx touched it.
///
/// Kept here as well as in the guard because **the release profile aborts on
/// panic**, so `Drop` never runs there and a panic hook is the only thing left
/// standing — and a hook cannot borrow a guard that lives on somebody's stack.
///
/// A `Mutex` rather than a channel or a thread-local: the hook may run on any
/// thread, and this is written once per session and read once per disaster.
static SAVED: Mutex<Option<String>> = Mutex::new(None);

/// Raw mode, and putting it back.
///
/// Carries nothing: the settings it will restore live in [`SAVED`], because the
/// panic hook has to reach them and cannot reach a guard on somebody's stack.
/// What this owns is the *moment* — dropping it is what puts them back on the
/// ordinary path.
#[derive(Debug)]
pub struct RawMode {
    _private: (),
}

impl RawMode {
    /// Puts the terminal in raw mode, or reports that it cannot be done.
    pub fn enter() -> Option<Self> {
        let saved = stty(&["-g"])?;

        // -echo so typing does not appear twice; -icanon so a keypress arrives
        // without waiting for Enter; -isig so Ctrl-C reaches us as a byte
        // rather than as a signal that would skip every restore below.
        stty(&["-echo", "-icanon", "-isig", "min", "1", "time", "0"])?;

        if let Ok(mut held) = SAVED.lock() {
            *held = Some(saved.trim().to_string());
        }

        Some(Self { _private: () })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        restore();
    }
}

/// Puts the terminal back the way it was found, from anywhere.
///
/// Idempotent, and safe to call when nothing was ever changed: the settings are
/// taken rather than copied, so the second call has nothing to do. That matters
/// because the ordinary path and the panic path both go through here, and on a
/// panic that unwinds they both run.
pub fn restore() {
    let saved = SAVED.lock().ok().and_then(|mut held| held.take());

    // Their settings, not `stty sane`. Sane restores *a* working terminal;
    // this restores the one somebody had, including whatever they had bound.
    if let Some(saved) = saved {
        let _ = stty(&[saved.as_str()]);
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
    fn a_pipe_means_nobody_is_there_to_press_a_key() {
        // Which is what makes `slidx open | head` print a list rather than wait
        // for a keypress in a CI job.
        assert!(!watching(false, None));
        assert!(watching(true, None));
    }

    #[test]
    fn a_shell_function_holding_the_output_is_not_a_pipe_with_nobody_behind_it() {
        // The integration captures stdout so it can follow a path. The person
        // is still at the terminal, and a picker that refused to draw would be
        // the integration making slidx worse.
        assert!(watching(false, Some("1")));
    }

    #[test]
    fn an_empty_variable_is_not_a_shell_function_holding_anything() {
        // Exported-but-empty is how a variable looks when a script unset it
        // badly, the same reading `NO_COLOR` gets.
        assert!(!watching(false, Some("")));
    }

    #[test]
    fn a_wheel_notch_is_decoded_from_an_sgr_mouse_report() {
        // `\x1b[<64;12;3M` — button 64 is a wheel up, 65 a wheel down, and the
        // two numbers after them are where the pointer was, which nothing here
        // cares about.
        assert_eq!(key_of(b"\x1b[<64;12;3M"), Key::ScrollUp);
        assert_eq!(key_of(b"\x1b[<65;12;3M"), Key::ScrollDown);
    }

    #[test]
    fn a_wheel_report_from_a_wide_window_is_still_one_keypress() {
        // The coordinates run to three digits and more on a large screen. A
        // decoder that stopped at a fixed length would leave `7M` behind, and
        // the `M` would arrive as a character somebody typed.
        assert_eq!(key_of(b"\x1b[<64;237;115M"), Key::ScrollUp);
    }

    #[test]
    fn a_mouse_button_nothing_is_bound_to_is_ignored_rather_than_guessed_at() {
        // A click, a drag, a middle button. Consumed whole either way, because
        // bytes left behind arrive as the next keypress.
        assert_eq!(key_of(b"\x1b[<0;10;5M"), Key::Ignored);
        assert_eq!(key_of(b"\x1b[<0;10;5m"), Key::Ignored);
    }

    #[test]
    fn a_terminal_that_goes_away_reports_an_interrupt_rather_than_spinning() {
        // Reading zero bytes forever is the difference between exiting and
        // pinning a core.
        assert_eq!(key_of(&[]), Key::Interrupt);
    }
}
