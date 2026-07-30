//! A `steps:` entry addressing a mark that is not there.
//!
//! This is the check worth the crate. A step whose target matches nothing is not
//! an error at any layer: the parser reads the action, the compiler puts it in
//! the timeline, the runtime queries the selector, finds no element, and moves
//! on. The stop is *there* — the presenter clicks and the slide does not change.
//!
//! It is found on stage, and it is found by a person who then has to decide
//! mid-sentence whether to click again.
//!
//! # What can and cannot be decided
//!
//! Only a target that names a **mark key** is checked. `#hero` is slidx's own
//! shorthand, expanded by `mark::resolve_target` into
//! `[data-slidx-mark="hero"]`, so the set of valid keys is exactly the marks on
//! that slide and the answer is certain.
//!
//! Everything else is left alone on purpose. `.highlight` is a theme class,
//! `li:nth-child(2)` is a CSS selector over markup a Markdown renderer produced,
//! and an island's step may target a node no build-time model has ever seen.
//! Guessing at those would report working decks, and a checker that cries wolf
//! is one an author switches off — taking this check with it.

use slidx_core::{Deck, Diagnostic, Diagnostics, Slide, SourceSpan, StepAction, MARK_ATTRIBUTE};

pub fn check(deck: &Deck, _installed: &crate::Installed, sink: &mut Diagnostics) {
    for slide in &deck.slides {
        let declared = keys(slide);

        for action in &slide.steps.actions {
            for target in action.targets() {
                let Some(key) = mark_key(target) else { continue };

                if !declared.iter().any(|declared| declared == key) {
                    sink.push(missing(slide, action, key, &declared));
                }
            }
        }
    }
}

/// Every mark key this slide declares.
///
/// Read off the slide's marks rather than its compiled markup, because the
/// markup also carries the keys the compiler invented for unkeyed marks — and a
/// generated key is not something a `steps:` entry may rely on. Two takes of one
/// value contribute one key, which is the point of a take.
fn keys(slide: &Slide) -> Vec<String> {
    slide.marks.iter().filter_map(|mark| mark.key.clone()).collect()
}

/// The mark key a target names, or `None` for a selector this cannot decide.
fn mark_key(target: &str) -> Option<&str> {
    target
        .strip_prefix(&format!("[{MARK_ATTRIBUTE}=\""))
        .and_then(|rest| rest.strip_suffix("\"]"))
        .filter(|key| !key.is_empty())
}

fn missing(slide: &Slide, action: &StepAction, key: &str, declared: &[String]) -> Diagnostic {
    let verb = verb(action);

    Diagnostic::warning(
        "dialect/unknown-target",
        format!("`{verb}` targets `#{key}`, and no mark on this slide declares it"),
    )
    .at(SourceSpan::line(slide.source_line).on_slide(slide.index))
    .with_help(help(key, declared))
}

/// What to do about it, which depends on whether the slide has marks at all.
///
/// A slide with `#result` on it and a step naming `#reuslt` is a typo, and naming
/// the keys that *are* there is the whole fix. A slide with no marks is a
/// different mistake — the author wrote the pipeline and not the mark — so
/// offering an empty list would read as slidx being broken.
fn help(key: &str, declared: &[String]) -> String {
    if declared.is_empty() {
        return format!(
            "mark the text this step is about: `[the words]{{#{key}}}`, \
             or target a theme class such as `.accent`"
        );
    }

    let offered: Vec<String> = declared.iter().map(|key| format!("`#{key}`")).collect();

    format!("this slide declares {}", offered.join(", "))
}

/// The word the author wrote, so the finding quotes their own line back.
fn verb(action: &StepAction) -> &'static str {
    match action {
        StepAction::Reveal { .. } => "reveal",
        StepAction::Hide { .. } => "hide",
        StepAction::Emphasize { .. } => "emphasize",
        StepAction::Set { .. } => "set",
        StepAction::Group { .. } => "group",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn found(source: &str) -> Diagnostics {
        let deck = parse_deck(source, &DeckParseOptions::default());
        let mut sink = Diagnostics::default();
        check(&deck, &crate::Installed::default(), &mut sink);
        sink
    }

    #[test]
    fn a_step_naming_a_mark_that_is_not_there_is_reported() {
        // Today the stop exists, the presenter clicks, and nothing on the slide
        // changes. They find out in front of the room.
        let sink =
            found("---\nsteps:\n  - reveal: \"#reuslt\"\n---\n\n# One\n\nThe [result]{#result}.\n");

        assert_eq!(sink.as_slice()[0].code, "dialect/unknown-target");
        assert!(sink.as_slice()[0].message.contains("#reuslt"));
        assert!(sink.as_slice()[0].message.contains("reveal"), "it quotes the author's own verb");
    }

    #[test]
    fn the_help_names_the_keys_the_slide_does_declare() {
        // Which is the fix for a typo: the author reads their own key back and
        // sees the two characters they swapped.
        let sink = found(
            "---\nsteps:\n  - reveal: \"#reuslt\"\n---\n\n\
             The [result]{#result} and the [cost]{#cost}.\n",
        );
        let help = sink.as_slice()[0].help.clone().unwrap();

        assert!(help.contains("`#result`"), "{help}");
        assert!(help.contains("`#cost`"), "{help}");
    }

    #[test]
    fn a_slide_with_no_marks_at_all_is_told_to_write_one() {
        // A different mistake: the pipeline was written and the mark was not.
        // Offering an empty list of keys would read as slidx being broken.
        let sink = found("---\nsteps:\n  - reveal: \"#hero\"\n---\n\n# One\n\nProse.\n");
        let help = sink.as_slice()[0].help.clone().unwrap();

        assert!(help.contains("[the words]{#hero}"), "{help}");
    }

    #[test]
    fn a_step_naming_a_mark_that_is_there_says_nothing() {
        assert!(found(
            "---\nsteps:\n  - reveal: \"#result\"\n---\n\n# One\n\nThe [result]{#result}.\n"
        )
        .is_empty());
    }

    #[test]
    fn two_takes_of_one_value_declare_one_key_between_them() {
        // The second take is lifted out of the markup and becomes a `Set` step,
        // so a check reading the compiled body would think the key had gone.
        assert!(found(
            "---\nsteps:\n  - emphasize: \"#latency\"\n---\n\n\
             Latency dropped to [120ms]{#latency}[38ms]{#latency}.\n"
        )
        .is_empty());
    }

    #[test]
    fn a_selector_this_cannot_decide_is_left_alone() {
        // A theme class, a CSS selector over rendered markup, an island's own
        // node. Guessing here would report working decks, and a checker that
        // cries wolf is one somebody switches off.
        for target in [".accent", "li:nth-child(2)", "#not-a-mark > span", "[data-x=\"1\"]"] {
            let source = format!("---\nsteps:\n  - reveal: \"{target}\"\n---\n\n# One\n\nProse.\n");
            assert!(found(&source).is_empty(), "{target} was reported");
        }
    }

    #[test]
    fn a_marker_derived_step_is_never_reported() {
        // `<!-- step -->` compiles to an anchor selector the parser invented, so
        // a check that read those as mark keys would flag every staged slide in
        // every deck.
        assert!(found("- one <!-- step -->\n- two <!-- step -->\n").is_empty());
        assert!(found("---\nautoSteps: list\n---\n\n- one\n- two\n").is_empty());
    }

    #[test]
    fn a_step_inside_a_group_is_checked_like_any_other() {
        // A group is where a typo hides: the other members of the group play, so
        // the slide does change and the missing one is easy to miss.
        let sink = found(
            "---\nsteps:\n  - group: [{ reveal: \"#a\" }, { reveal: \"#gone\" }]\n---\n\n\
             The [a]{#a}.\n",
        );

        assert_eq!(sink.len(), 1);
        assert!(sink.as_slice()[0].message.contains("#gone"));
    }

    #[test]
    fn a_key_written_in_japanese_is_matched_as_written() {
        // Keys are the author's words, and this maintainer's decks are in
        // Japanese. A byte comparison is the only one that can be right here.
        assert!(found(
            "---\nsteps:\n  - reveal: \"#結果\"\n---\n\n結果は [3.2倍速く]{#結果} なった。\n"
        )
        .is_empty());

        let sink = found(
            "---\nsteps:\n  - reveal: \"#結論\"\n---\n\n結果は [3.2倍速く]{#結果} なった。\n",
        );
        assert_eq!(sink.len(), 1);
        assert!(sink.as_slice()[0].message.contains("#結論"));
    }

    #[test]
    fn a_target_on_one_slide_is_not_satisfied_by_a_mark_on_another() {
        // Steps are per-slide and so is the DOM they query. A key that exists
        // two slides later is not there when the presenter clicks.
        let sink = found(
            "---\nsteps:\n  - reveal: \"#later\"\n---\n\n# One\n\n---\n\nThe [later]{#later}.\n",
        );

        assert_eq!(sink.len(), 1);
        assert_eq!(sink.as_slice()[0].span.slide_index, Some(0));
    }

    #[test]
    fn a_finding_points_at_the_slide_that_carries_the_pipeline() {
        let sink = found("# One\n\n---\nsteps:\n  - reveal: \"#gone\"\n---\n\n# Two\n");

        assert_eq!(sink.as_slice()[0].span.slide_index, Some(1));
        assert!(sink.as_slice()[0].span.line > 1);
    }

    #[test]
    fn a_mark_selector_is_recognised_and_nothing_else_is() {
        assert_eq!(mark_key("[data-slidx-mark=\"hero\"]"), Some("hero"));
        assert_eq!(mark_key("[data-slidx-mark=\"\"]"), None, "an empty key names nothing");
        assert_eq!(mark_key("[data-slidx-step=\"1\"]"), None);
        assert_eq!(mark_key(".accent"), None);
    }
}
