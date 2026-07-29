//! Whether the fonts the deck's theme names exist on this machine.
//!
//! Two failures, and they need different advice. Landing on a *later* member of
//! the stack means the deck renders in a face it was not laid out against —
//! wider, and the title that fitted on one line now takes two. Landing on
//! *nothing* means the browser picks its own default, which differs by platform
//! and cannot be predicted from the deck at all.
//!
//! Neither is fixable at the venue by installing a font, so the remedy is
//! always about the deck: switch to a theme with a system stack, or look at the
//! slides that will be affected before you present them. That is worth saying
//! out loud, because a remedy the speaker cannot perform in the room is the
//! same as no remedy.

use crate::environment::fonts::Resolution;
use crate::environment::{Environment, FontStack, InstalledFonts};
use crate::finding::Finding;

const ID: &str = "fonts";

pub fn check(environment: &Environment) -> Finding {
    // An empty stack is a theme that named a role and no families for it.
    // There is nothing to resolve, so it cannot fail — dropping it here keeps
    // the message from listing a stack with nothing in the brackets.
    let stacks: Vec<&FontStack> =
        environment.expected.fonts.iter().filter(|stack| !stack.families.is_empty()).collect();

    if stacks.is_empty() {
        return Finding::pass(ID, "the deck's theme names no font stacks");
    }

    let Some(installed) = environment.fonts.value() else {
        return without_a_font_list(environment, &stacks);
    };

    let missing = stacks_where(&stacks, installed, |resolution| resolution == Resolution::Missing);
    if !missing.is_empty() {
        return Finding::fail(
            ID,
            format!(
                "nothing resolves in {}: {}",
                plural(missing.len(), "stack"),
                describe(&missing)
            ),
            "the browser will substitute a default that differs by platform, so the deck's line \
             lengths stop meaning anything. Switch to a theme with a system font stack, or add a \
             generic family such as `sans-serif` to the end of the stack",
        );
    }

    let substituted = stacks_where(&stacks, installed, |resolution| {
        matches!(resolution, Resolution::Fallback { .. })
    });
    if !substituted.is_empty() {
        return Finding::warn(
            ID,
            format!(
                "{} will be substituted: {}",
                plural(substituted.len(), "stack"),
                describe(&substituted)
            ),
            "you cannot install a font in the two minutes before a talk. Look at the title slide \
             and the densest slide now — a substituted face is wider, and what fitted on one line \
             may not any more",
        );
    }

    Finding::pass(
        ID,
        format!("every family the theme names resolves here ({} installed)", installed.len()),
    )
}

/// The font list could not be read.
///
/// Not automatically an unknown: a stack that *starts* with a generic family
/// resolves on every machine by definition, so its answer does not depend on
/// the reading that failed. Every slidx built-in theme is in that shape, which
/// is the reason this check stays green on a platform whose fonts cannot be
/// enumerated.
fn without_a_font_list<'a>(environment: &Environment, stacks: &[&'a FontStack]) -> Finding {
    let unverifiable: Vec<&'a FontStack> = stacks
        .iter()
        .copied()
        .filter(|stack| {
            stack.resolve(&InstalledFonts::default()) != Resolution::Preferred(first(stack))
        })
        .collect();

    if unverifiable.is_empty() {
        return Finding::pass(
            ID,
            "every stack the theme names leads with a generic family, which resolves everywhere",
        );
    }

    Finding::unknown(
        ID,
        format!(
            "the installed fonts could not be listed ({}), so {} could not be checked: {}",
            environment.fonts.reason().unwrap_or("no reason given"),
            plural(unverifiable.len(), "stack"),
            describe(&unverifiable)
        ),
        "open the deck and look at a heading. If the letterforms are not the ones you designed \
         against, switch to a theme with a system font stack",
    )
}

fn first(stack: &FontStack) -> String {
    stack.families.first().cloned().unwrap_or_default()
}

fn stacks_where<'a>(
    stacks: &[&'a FontStack],
    installed: &InstalledFonts,
    predicate: impl Fn(Resolution) -> bool,
) -> Vec<&'a FontStack> {
    stacks.iter().copied().filter(|stack| predicate(stack.resolve(installed))).collect()
}

fn describe(stacks: &[&FontStack]) -> String {
    stacks
        .iter()
        .map(|stack| format!("{} ({})", stack.role, stack.label()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{Expectation, Reading};
    use crate::finding::Status;

    fn machine(installed: &[&str], stacks: &[(&str, &str)]) -> Environment {
        let expected = stacks.iter().fold(Expectation::default(), |expected, (role, css)| {
            expected.with_font_stack(*role, css)
        });

        Environment::new()
            .with_fonts(Reading::known(installed.iter().collect::<InstalledFonts>()))
            .expecting(expected)
    }

    #[test]
    fn a_theme_whose_fonts_are_all_installed_passes() {
        let environment = machine(
            &["Inter", "IBM Plex Mono"],
            &[("sans", "Inter, sans-serif"), ("mono", "'IBM Plex Mono', monospace")],
        );

        assert_eq!(check(&environment).status, Status::Pass);
    }

    #[test]
    fn a_theme_that_names_no_fonts_has_nothing_that_can_break() {
        let environment = Environment::new().with_fonts(Reading::known(InstalledFonts::default()));

        assert_eq!(check(&environment).status, Status::Pass);
    }

    #[test]
    fn a_first_choice_that_is_missing_warns_rather_than_fails() {
        // The deck still renders, in the wrong face. That is a "go and look at
        // the title slide" problem, not a "do not start" problem.
        let environment = machine(&["Arial"], &[("sans", "Inter, Arial, sans-serif")]);
        let finding = check(&environment);

        assert_eq!(finding.status, Status::Warn);
        assert!(finding.detail.contains("sans"), "got: {}", finding.detail);
    }

    #[test]
    fn the_substitution_warning_tells_the_speaker_which_slides_to_look_at() {
        // Nobody can install a font at a lectern, so the only useful advice is
        // about what to check.
        let environment = machine(&["Arial"], &[("sans", "Inter, Arial, sans-serif")]);
        let remedy = check(&environment).remedy.unwrap();

        assert!(remedy.contains("title slide"), "got: {remedy}");
    }

    #[test]
    fn a_stack_that_resolves_to_nothing_fails() {
        // No generic at the end, nothing installed: the browser picks a
        // default that differs by platform and the layout is unpredictable.
        let environment = machine(&["Arial"], &[("display", "Söhne, 'GT America'")]);

        assert_eq!(check(&environment).status, Status::Fail);
    }

    #[test]
    fn a_failing_stack_is_told_to_add_a_generic_family() {
        // The durable fix, and one the author can apply the moment they are
        // back at a desk.
        let environment = machine(&[], &[("display", "Söhne")]);
        let remedy = check(&environment).remedy.unwrap();

        assert!(remedy.contains("sans-serif"), "got: {remedy}");
    }

    #[test]
    fn a_missing_stack_outranks_a_merely_substituted_one() {
        // One line per check means the worse of the two has to win, or the
        // speaker acts on the smaller problem.
        let environment = machine(&["Arial"], &[("sans", "Inter, Arial"), ("display", "Söhne")]);

        assert_eq!(check(&environment).status, Status::Fail);
    }

    #[test]
    fn every_affected_stack_is_named_by_role() {
        // "A font is missing" is not actionable. "The mono stack is missing"
        // tells the speaker to go and look at the code slides.
        let environment =
            machine(&[], &[("sans", "Inter, sans-serif"), ("mono", "'IBM Plex Mono', monospace")]);
        let detail = check(&environment).detail;

        assert!(detail.contains("sans"), "got: {detail}");
        assert!(detail.contains("mono"), "got: {detail}");
        assert!(detail.contains("2 stacks"), "got: {detail}");
    }

    #[test]
    fn an_unlistable_font_directory_still_passes_a_theme_that_leads_with_a_generic() {
        // `system-ui` resolves everywhere by definition, so the answer does not
        // depend on the reading that failed. Every built-in slidx theme is in
        // this shape, which is why this is worth the extra branch.
        let environment = Environment::new()
            .with_fonts(Reading::unavailable("fc-list is not installed"))
            .expecting(Expectation::default().with_font_stack("sans", "system-ui, sans-serif"));

        assert_eq!(check(&environment).status, Status::Pass);
    }

    #[test]
    fn an_unlistable_font_directory_is_unknown_when_the_theme_names_a_real_family() {
        // Inter may well be installed. We do not know, and saying "pass" here
        // is exactly the false green this crate exists to avoid.
        let environment = Environment::new()
            .with_fonts(Reading::unavailable("fc-list is not installed"))
            .expecting(Expectation::default().with_font_stack("sans", "Inter, sans-serif"));
        let finding = check(&environment);

        assert_eq!(finding.status, Status::Unknown);
        assert!(finding.detail.contains("fc-list is not installed"));
        assert!(finding.remedy.is_some());
    }

    #[test]
    fn a_stack_that_names_no_families_is_ignored_rather_than_failed() {
        // A theme can declare a role and leave it empty. There is nothing to
        // resolve, so failing it would report a problem that does not exist.
        let environment = machine(&["Inter"], &[("display", ""), ("sans", "Inter")]);

        assert_eq!(check(&environment).status, Status::Pass);
    }

    #[test]
    fn counts_are_written_in_singular_and_plural() {
        assert_eq!(plural(1, "stack"), "1 stack");
        assert_eq!(plural(3, "stack"), "3 stacks");
    }
}
