//! The `slidx` binary.
//!
//! Everything interesting is in the library next to this file, so it can all be
//! tested without a process. This is the shell: read the arguments, decide once
//! whether the terminal will take colour, write, exit.

use std::io::Write;
use std::process::ExitCode;

use slidx_cli::style::Style;

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let outcome = slidx_cli::run(&argv, &Style::detect());

    // Written with `write!` and not `print!` so a closed pipe — `slidx doctor
    // | head` — ends the process quietly instead of panicking inside the
    // formatting machinery, which would print a Rust backtrace at a lectern.
    let _ = write!(std::io::stdout(), "{}", outcome.stdout);
    let _ = write!(std::io::stderr(), "{}", outcome.stderr);

    ExitCode::from(outcome.code)
}
