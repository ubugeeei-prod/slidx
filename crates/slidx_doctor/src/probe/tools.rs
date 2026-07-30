//! How a probe reaches a platform's own tool.
//!
//! The readings below it — the displays, the Do Not Disturb state, the output
//! level — exist on one operating system each and nowhere else. `xrandr` is not
//! installed on a Mac, `system_profiler` is not a thing on Linux, and neither
//! is anywhere near a Windows runner. Written as `#[cfg]` branches, two of
//! every three would be dead on any one machine, and a branch nothing runs is a
//! branch that breaks silently until somebody is standing in a room with it.
//!
//! So the tool runner is a value. [`Tools::on_this_machine`] runs the real
//! thing with the deadline every reading here has; a test hands over the exact
//! bytes `system_profiler` prints and drives the macOS branch on Linux. Every
//! platform's parsing is then exercised on every platform CI has, which is what
//! makes "each of these has a test" a claim the suite holds rather than one the
//! docs make.
//!
//! There is no filesystem here on purpose. The one reading that comes from a
//! file — macOS keeps the Focus state in one — takes its directory as an
//! argument instead, the way the sysfs battery walk already does, so a fixture
//! stands in for a home directory.

use std::fmt;
use std::time::Duration;

use crate::environment::Reading;
use crate::probe::command;

/// The platform tools a probe is allowed to run.
pub struct Tools<'a> {
    run: Box<dyn Fn(&str, &[&str]) -> Result<String, String> + 'a>,
}

impl Tools<'static> {
    /// Runs the real tools, each under the request's deadline.
    pub fn on_this_machine(timeout: Duration) -> Self {
        Self::answering(move |program, args| command::output(program, args, timeout))
    }

    /// A machine where none of the tools a probe asks for exist.
    ///
    /// The locked-down laptop, and the shape of every platform branch running
    /// somewhere it was not written for.
    pub fn absent() -> Self {
        Self::answering(|program, _| Err(format!("`{program}` could not be run")))
    }
}

impl<'a> Tools<'a> {
    /// Answers from a closure rather than from the machine.
    ///
    /// Public because the tests are not the only caller that wants it: an
    /// embedder replaying a captured machine, or reading one over a wire, has
    /// the same need the crate's `Environment` builder already serves.
    pub fn answering(
        run: impl Fn(&str, &[&str]) -> Result<String, String> + 'a,
    ) -> Self {
        Self { run: Box::new(run) }
    }

    pub fn output(&self, program: &str, args: &[&str]) -> Result<String, String> {
        (self.run)(program, args)
    }
}

impl fmt::Debug for Tools<'_> {
    /// Names the type and nothing else. What a `Tools` holds is a closure, and
    /// printing a machine's captured output into a debug line would put a
    /// screenful of `system_profiler` inside one field of an `Environment`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Tools")
    }
}

/// Turns a tool's output into a reading, or says why there is none.
///
/// Three outcomes and they are all different sentences: the tool would not run,
/// the tool ran and said something this build cannot read, or a reading. The
/// middle one is the one worth keeping separate — it is how a platform that
/// changed the shape of its output shows up, and calling it "not installed"
/// would send somebody looking for the wrong thing.
pub fn parsed<T>(
    output: Result<String, String>,
    parse: impl Fn(&str) -> Option<T>,
    unreadable: &str,
) -> Reading<T> {
    match output {
        Ok(text) => match parse(&text) {
            Some(value) => Reading::known(value),
            None => Reading::unavailable(unreadable.to_string()),
        },
        Err(reason) => Reading::unavailable(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stubbed_tool_answers_with_whatever_the_test_handed_over() {
        // The seam itself. Without this every platform branch below would only
        // ever run on the one runner whose tools exist.
        let tools = Tools::answering(|program, args| Ok(format!("{program} {}", args.join(" "))));

        assert_eq!(tools.output("xrandr", &["--query"]), Ok("xrandr --query".to_string()));
    }

    #[test]
    fn a_machine_with_none_of_the_tools_installed_reports_which_one_was_missing() {
        // The reason ends up in front of a speaker as the reason a line reads
        // unknown, so it has to name the thing that was not there.
        let error = Tools::absent().output("pactl", &[]).err().unwrap_or_default();

        assert!(error.contains("pactl"), "got: {error}");
    }

    #[test]
    fn a_tool_that_would_not_run_becomes_the_reason_the_reading_is_missing() {
        let reading: Reading<u8> =
            parsed(Err("`xrandr` could not be run".to_string()), |_| Some(1), "unreadable");

        assert_eq!(reading.reason(), Some("`xrandr` could not be run"));
    }

    #[test]
    fn a_tool_that_answered_with_something_unreadable_says_so_rather_than_missing() {
        // A platform that changed the shape of its output is a different
        // problem from a platform that has no such tool, and a speaker sent
        // looking for an uninstalled program would never find it.
        let reading: Reading<u8> =
            parsed(Ok("something new".to_string()), |_| None, "this build cannot read that");

        assert_eq!(reading.reason(), Some("this build cannot read that"));
    }

    #[test]
    fn a_tool_that_answered_something_readable_becomes_a_reading() {
        let reading = parsed(Ok("7".to_string()), |text| text.trim().parse::<u8>().ok(), "no");

        assert_eq!(reading.value(), Some(&7));
    }

    #[test]
    fn the_runner_prints_as_a_name_rather_than_as_a_machines_output() {
        // An Environment is debug-printed into bug reports. A captured
        // screenful of `system_profiler` inside one field would drown it.
        assert_eq!(format!("{:?}", Tools::absent()), "Tools");
    }
}
