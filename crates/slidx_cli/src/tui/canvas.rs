//! Borrowing the terminal, and giving it back whatever happens.
//!
//! # The failure everybody remembers
//!
//! A terminal left in raw mode has no echo and no line editing. Typing produces
//! nothing, Enter does nothing, and it looks exactly like a hung machine — so
//! the person closes the window, and what they remember about the tool is that
//! it broke their shell. That is the failure this module exists to prevent, and
//! it has to be prevented on **every** way out, not on the tidy one.
//!
//! There are four ways out and each is closed by something different:
//!
//! | how it ends | what puts the terminal back |
//! | ----------- | --------------------------- |
//! | quitting, or the end of the deck | [`Session`]'s `Drop` |
//! | an early return, or an unwinding panic | the same `Drop` |
//! | a panic in a release build | the panic hook, because `panic = "abort"` means `Drop` never runs |
//! | Ctrl-C | never becomes a signal at all: `stty -isig` delivers it as a byte |
//!
//! The third row is the one worth pausing on. This workspace's release profile
//! aborts on panic, so the destructor that restores the terminal in every test
//! and every debug build **does not run in the binary people install**. The hook
//! is not belt and braces; it is the only strap on that path.
//!
//! A `SIGTERM` or a `SIGKILL` from elsewhere is beyond reach without a signal
//! handling dependency, and saying so is better than implying otherwise.
//!
//! ## Why the alternate screen
//!
//! The view takes the whole window and hands it back untouched: the shell's
//! scrollback is exactly where it was, with no half-erased frames in it. That
//! also removes the redraw's worst problem — a frame drawn where the previous
//! one has just been erased flickers, and there is nothing to erase here.
//!
//! ## No flicker, without double buffering
//!
//! Each frame is one write, and the frame does not clear the screen first. The
//! cursor goes home and every line is drawn over what was there, clearing only
//! from the end of that line — so no cell is ever briefly blank. A terminal is
//! already double buffered by its own compositor; what makes a redraw flicker is
//! clearing and then drawing, not the drawing.

use std::fs::File;
use std::io::Write;

use crate::terminal::{self, RawMode};

/// The escape sequences, named so a reader can look each one up.
mod sequence {
    /// Switch to the alternate screen buffer, and back.
    pub const ALTERNATE_ON: &str = "\u{1b}[?1049h";
    pub const ALTERNATE_OFF: &str = "\u{1b}[?1049l";
    /// A cursor parked in a view nobody is typing into is a distraction that
    /// blinks.
    pub const CURSOR_HIDE: &str = "\u{1b}[?25l";
    pub const CURSOR_SHOW: &str = "\u{1b}[?25h";
    /// Button events, in SGR encoding. The older encoding packs a coordinate
    /// into one byte and cannot describe a window wider than 223 columns.
    pub const MOUSE_ON: &str = "\u{1b}[?1000h\u{1b}[?1006h";
    pub const MOUSE_OFF: &str = "\u{1b}[?1006l\u{1b}[?1000l";
    /// Home, clear to the end of the line, clear to the end of the screen.
    pub const HOME: &str = "\u{1b}[H";
    pub const CLEAR_LINE: &str = "\u{1b}[K";
    pub const CLEAR_BELOW: &str = "\u{1b}[J";
}

/// Everything written on the way in, and its mirror on the way out.
///
/// Assembled here rather than at two call sites so the pairs cannot drift: a
/// view that turned mouse reporting on and forgot to turn it off leaves every
/// later command in that terminal printing escape sequences when the wheel
/// moves.
fn opening() -> String {
    format!("{}{}{}", sequence::ALTERNATE_ON, sequence::CURSOR_HIDE, sequence::MOUSE_ON)
}

fn closing() -> String {
    format!("{}{}{}", sequence::MOUSE_OFF, sequence::CURSOR_SHOW, sequence::ALTERNATE_OFF)
}

/// One frame, as the bytes that put it on screen.
///
/// Separated from the writing so the interesting half is a pure function a test
/// can read — a `Session` cannot be built without taking over the terminal the
/// suite is running in.
///
/// Nothing here clears the screen. The cursor goes home and each line is drawn
/// over what was there, clearing only from its own end, so no cell is briefly
/// blank; clearing and then drawing is what makes a redraw flicker.
///
/// Lines end `\r\n` rather than `\n`: raw mode is exactly the absence of the
/// translation that would have moved the carriage, so a frame written with bare
/// newlines walks diagonally down the screen.
fn painted(frame: &str) -> String {
    let mut out = String::with_capacity(frame.len() + 64);
    out.push_str(sequence::HOME);

    for line in frame.lines() {
        out.push_str(line);
        out.push_str(sequence::CLEAR_LINE);
        out.push_str("\r\n");
    }

    // Whatever a taller previous frame left below this one. Done last, so no
    // part of the screen is ever blank between two frames.
    out.push_str(sequence::CLEAR_BELOW);
    out
}

/// The terminal, for as long as the view is on it.
#[derive(Debug)]
pub struct Session {
    tty: File,
    /// Dropped after the escape sequences below it, because raw mode has to
    /// outlive the writes that depend on it.
    _raw: RawMode,
}

impl Session {
    /// Takes the terminal, or reports that this one cannot be driven.
    ///
    /// `None` on a terminal with no `stty` — Windows, a stripped container — and
    /// the caller falls back to printing one frame. That is a degradation rather
    /// than an error: the command still answers.
    pub fn open() -> Option<Self> {
        let mut tty = terminal::open()?;
        let raw = RawMode::enter()?;

        install_the_panic_hook();

        let _ = write!(tty, "{}", opening());
        let _ = tty.flush();

        Some(Self { tty, _raw: raw })
    }

    /// Draws one frame, in one write, over the last one.
    pub fn paint(&mut self, frame: &str) {
        let _ = write!(self.tty, "{}", painted(frame));
        let _ = self.tty.flush();
    }

    /// Waits for one keypress.
    pub fn read(&mut self) -> terminal::Key {
        terminal::read_key(&mut self.tty)
    }

    /// The window's size in cells, asked of the terminal every time.
    ///
    /// Every frame, which is what makes the view follow a resize: there is no
    /// `SIGWINCH` here to wake on without a dependency, and a size read once at
    /// startup is a view that ignores the window from then on.
    pub fn size(&self) -> Option<(usize, usize)> {
        terminal::size()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = write!(self.tty, "{}", closing());
        let _ = self.tty.flush();
    }
}

/// Restores the terminal from a panic hook, where nothing can be borrowed.
///
/// Writes to the terminal by name rather than through the session: the session
/// is being unwound, or in a release build is not being unwound at all.
fn install_the_panic_hook() {
    use std::sync::Once;

    static ONCE: Once = Once::new();

    ONCE.call_once(|| {
        let previous = std::panic::take_hook();

        std::panic::set_hook(Box::new(move |panic| {
            // The terminal first, then the message. The other order prints the
            // panic onto the alternate screen, which is then thrown away — so
            // the crash is invisible and all anybody has is a tool that quit.
            if let Ok(mut tty) = File::options().write(true).open(terminal::TTY) {
                let _ = write!(tty, "{}", closing());
                let _ = tty.flush();
            }

            terminal::restore();
            previous(panic);
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_turned_on_is_turned_off_again() {
        // The pairs are the whole contract. Mouse reporting left on is the
        // cruellest one: every later command in that terminal prints escape
        // sequences when the wheel moves, and nothing says why.
        for (on, off) in [
            (sequence::ALTERNATE_ON, sequence::ALTERNATE_OFF),
            (sequence::CURSOR_HIDE, sequence::CURSOR_SHOW),
            (sequence::MOUSE_ON, sequence::MOUSE_OFF),
        ] {
            assert!(opening().contains(on), "{on:?} is never turned on");
            assert!(closing().contains(off), "{off:?} is never turned off");
        }
    }

    #[test]
    fn the_way_out_undoes_the_way_in_in_the_opposite_order() {
        // Mouse reporting is turned off before the screen is handed back, so
        // the sequence is not written onto the shell's own screen.
        let closing = closing();
        let mouse = closing.find(sequence::MOUSE_OFF).expect("mouse off");
        let screen = closing.find(sequence::ALTERNATE_OFF).expect("alternate off");

        assert!(mouse < screen, "{closing:?}");
    }

    #[test]
    fn a_frame_is_drawn_over_the_last_one_rather_than_after_clearing_it() {
        // Clearing and then drawing is what makes a redraw flicker. Nothing
        // here clears the screen: the cursor goes home and each line clears
        // only its own tail as it is written.
        let painted = painted("one\ntwo");

        assert!(painted.starts_with(sequence::HOME), "{painted:?}");
        assert!(!painted.contains("\u{1b}[2J"), "the screen is cleared: {painted:?}");
        assert_eq!(painted.matches(sequence::CLEAR_LINE).count(), 2, "{painted:?}");
    }

    #[test]
    fn a_shorter_frame_clears_what_the_taller_one_left_below_it() {
        // Walking from a slide with twelve bullets to one with two would
        // otherwise leave ten rows of the previous slide on screen.
        assert!(painted("one").ends_with(sequence::CLEAR_BELOW));
    }

    #[test]
    fn a_frame_is_one_write_because_two_can_be_seen_between() {
        // Not asserted on the write itself — asserted by the shape of what is
        // handed to it. A `paint` that wrote line by line would show the frame
        // assembling itself over a slow link, which is the ssh case.
        assert_eq!(painted("one\ntwo").matches(sequence::HOME).count(), 1);
    }

    #[test]
    fn every_line_returns_the_carriage_as_well_as_advancing() {
        // Raw mode is the absence of the translation that would have done it.
        // With bare newlines the frame walks diagonally down the screen, which
        // is the first thing anybody sees go wrong here.
        let painted = painted("one\ntwo");

        assert_eq!(painted.matches("\r\n").count(), 2, "{painted:?}");
        assert!(!painted.contains("K\n\u{1b}"), "a line ended without a return: {painted:?}");
    }
}
