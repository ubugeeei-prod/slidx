//! Running a command that is not allowed to hang.
//!
//! Several readings are only available by asking a platform tool — `df`,
//! `pmset`, `fc-list`, PowerShell. Any of them can wedge: a stale network
//! mount makes `df` block indefinitely, and a PowerShell that decides to load a
//! profile can take longer than the talk's introduction.
//!
//! [`std::process`] has no timeout, so this module supplies one. It is the only
//! place in the crate that knows how to start a subprocess, which means the
//! deadline cannot be forgotten at a call site.
//!
//! The output pipe is drained on a separate thread on purpose. Waiting for a
//! child to exit while its output fills a pipe nobody is reading deadlocks the
//! two against each other — a hang with no error, which is the exact failure
//! this module exists to prevent.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// How often the child is checked for having exited. Short enough that a fast
/// command is not made slow by the polling, long enough not to spin a core.
const POLL: Duration = Duration::from_millis(20);

/// Runs a command and returns its standard output.
///
/// The error is written for a speaker, not for a log: it ends up inside the
/// `Unknown` finding as the reason the reading is missing.
pub fn output(program: &str, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("`{program}` could not be run ({error})"))?;

    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill();
        return Err(format!("`{program}` gave no output stream"));
    };

    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut buffer = String::new();
        let _ = stdout.read_to_string(&mut buffer);
        let _ = sender.send(buffer);
    });

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            // A non-zero exit is a reading we did not get. `pmset` on a
            // machine with no battery and `fc-list` that is not installed both
            // land here, and both are legitimate answers rather than bugs.
            Ok(Some(status)) => return Err(format!("`{program}` exited with {status}")),
            Ok(None) => {
                if started.elapsed() >= timeout {
                    // Killed rather than waited on: a wedged `df` against a
                    // dead network mount never returns, and the speaker needs
                    // the other six readings more than this one.
                    let _ = child.kill();
                    return Err(format!(
                        "`{program}` did not answer within {:.0}s",
                        timeout.as_secs_f32()
                    ));
                }
                thread::sleep(POLL);
            }
            Err(error) => return Err(format!("`{program}` could not be waited on ({error})")),
        }
    }

    receiver
        .recv_timeout(timeout)
        .map_err(|_| format!("`{program}` finished without producing readable output"))
}

/// Runs a command and returns its output, or `None`.
///
/// For the callers that only want the text and have their own way of saying
/// why a reading is missing.
pub fn try_output(program: &str, args: &[&str], timeout: Duration) -> Option<String> {
    output(program, args, timeout).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shell commands differ; every platform's own is used to prove the same
    /// behaviour, so this file's contract is tested on all three rather than
    /// asserted on one and hoped for on the others.
    fn sleeper(seconds: u32) -> (&'static str, Vec<String>) {
        #[cfg(windows)]
        {
            ("cmd", vec!["/C".into(), format!("ping -n {} 127.0.0.1 > nul", seconds + 1)])
        }
        #[cfg(not(windows))]
        {
            ("sleep", vec![seconds.to_string()])
        }
    }

    fn echoer(text: &str) -> (&'static str, Vec<String>) {
        #[cfg(windows)]
        {
            ("cmd", vec!["/C".into(), format!("echo {text}")])
        }
        #[cfg(not(windows))]
        {
            ("echo", vec![text.to_string()])
        }
    }

    fn run(command: (&'static str, Vec<String>), timeout: Duration) -> Result<String, String> {
        let (program, args) = command;
        let args: Vec<&str> = args.iter().map(String::as_str).collect();

        output(program, &args, timeout)
    }

    #[test]
    fn a_command_that_answers_returns_its_output() {
        let result = run(echoer("hello"), Duration::from_secs(5));

        assert_eq!(result.map(|output| output.trim().to_string()), Ok("hello".to_string()));
    }

    #[test]
    fn a_command_that_hangs_is_killed_and_reported_rather_than_waited_for() {
        // The behaviour the whole module exists for. A wedged `df` against a
        // dead network mount must cost one reading, not the report.
        let started = Instant::now();
        let result = run(sleeper(30), Duration::from_millis(200));

        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(10), "took {:?}", started.elapsed());
    }

    #[test]
    fn the_timeout_message_says_which_command_gave_up() {
        // It is shown to the speaker as the reason a line reads `unknown`.
        let error = run(sleeper(30), Duration::from_millis(100)).err().unwrap_or_default();

        assert!(error.contains("did not answer"), "got: {error}");
    }

    #[test]
    fn a_command_that_does_not_exist_is_an_error_and_not_a_panic() {
        // Every platform tool this crate reaches for is missing somewhere.
        let error =
            output("slidx-no-such-command", &[], Duration::from_secs(1)).err().unwrap_or_default();

        assert!(error.contains("could not be run"), "got: {error}");
    }

    #[test]
    fn a_command_that_fails_is_reported_rather_than_returning_empty_output() {
        // `pmset` on a machine with no battery exits non-zero. Treating that
        // as an empty successful reading would parse to "no battery found",
        // which is a different claim.
        let result = output("cargo", &["--slidx-not-a-flag"], Duration::from_secs(10));

        assert!(result.is_err());
    }

    #[test]
    fn try_output_gives_nothing_rather_than_an_error_for_the_callers_that_want_that() {
        assert!(try_output("slidx-no-such-command", &[], Duration::from_secs(1)).is_none());
    }
}
