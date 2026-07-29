//! Demo rules.
//!
//! A live demo is the one thing in a talk that can fail completely while the
//! room watches. The deck can declare a recording to switch to, and the whole
//! value of that declaration is that it was made *early* — a fallback is
//! useless unless it already exists, already ships with the deck, and is
//! already loaded before the demo is reached.
//!
//! All three of those are desk-time facts, which is why they are checked here.
//! Nothing on stage can tell an author they never recorded the video.

use slidx_core::{Diagnostic, Diagnostics, Severity, SourceSpan};

// Borrowed from the offline rule rather than reimplemented. "Does this need
// the network" is one question, and a fallback video judged local by one rule
// and remote by the other would be a deck that lints clean and dies on stage.
use crate::rules::offline::is_remote;
use crate::{LintInput, LintOptions};

pub fn check(input: &LintInput<'_>, _options: &LintOptions, sink: &mut Diagnostics) {
    for slide in &input.deck.slides {
        let Some(demo) = &slide.demo else { continue };
        let span = SourceSpan::line(slide.source_line).on_slide(slide.index);

        if !demo.has_fallback() {
            sink.push(
                Diagnostic::new(
                    "demo/no-fallback",
                    Severity::Warning,
                    format!(
                        "\"{}\" drives a live demo with no recording to fall back to",
                        slide.display_title()
                    ),
                )
                .at(span)
                .with_help("add `fallback: ./demo.mp4` — a recording of the demo working"),
            );
            continue;
        }

        // Only reached when a fallback exists, so an author who has recorded
        // nothing gets one instruction rather than two.
        let Some(fallback) = &demo.fallback else { continue };

        if is_remote(fallback) {
            sink.push(
                Diagnostic::new(
                    "demo/remote-fallback",
                    Severity::Error,
                    format!(
                        "the fallback for \"{}\" is fetched over the network",
                        slide.display_title()
                    ),
                )
                .at(span)
                .with_help(
                    "the demo fails when the network does — ship the recording with the deck",
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use slidx_core::{Diagnostic, Severity};

    use crate::test_support::lint_deck;

    fn demos(source: &str) -> Vec<Diagnostic> {
        lint_deck(source).into_iter().filter(|d| d.code.starts_with("demo/")).collect()
    }

    #[test]
    fn a_demo_with_a_recorded_fallback_is_what_the_rule_is_looking_for() {
        let source = "---\ndemo:\n  live: https://app.example.com\n  fallback: ./checkout.mp4\n---\n\n# Live\n";
        assert!(demos(source).is_empty());
    }

    #[test]
    fn a_demo_with_no_recording_is_reported_before_the_author_leaves_their_desk() {
        let source = "---\ndemo: https://app.example.com\n---\n\n# Checkout\n";

        let first = demos(source).into_iter().find(|d| d.code == "demo/no-fallback").unwrap();
        assert_eq!(first.severity, Severity::Warning);
        assert!(first.message.contains("Checkout"), "got: {}", first.message);
    }

    #[test]
    fn the_missing_fallback_is_pinned_to_the_slide_that_declares_the_demo() {
        let source = "# One\n\n---\ndemo: https://app.example.com\n---\n\n# Two\n";

        let first = demos(source).into_iter().find(|d| d.code == "demo/no-fallback").unwrap();
        assert_eq!(first.span.slide_index, Some(1));
    }

    #[test]
    fn the_help_names_the_key_that_records_the_fallback() {
        let source = "---\ndemo: https://app.example.com\n---\n\n# Checkout\n";

        let first = demos(source).into_iter().find(|d| d.code == "demo/no-fallback").unwrap();
        assert!(first.help.as_ref().unwrap().contains("fallback"));
    }

    #[test]
    fn a_fallback_that_has_to_be_fetched_is_not_a_fallback() {
        // The moment the fallback is needed is the moment the network is gone,
        // so a remote recording fails in exactly the situation it exists for.
        let source = "---\ndemo:\n  live: https://app.example.com\n  fallback: https://cdn.example.com/checkout.mp4\n---\n\n# Live\n";

        let first = demos(source).into_iter().find(|d| d.code == "demo/remote-fallback").unwrap();
        assert_eq!(first.severity, Severity::Error);
    }

    #[test]
    fn a_fallback_served_by_a_dev_server_is_still_a_fetch() {
        let source = "---\ndemo:\n  live: https://app.example.com\n  fallback: http://localhost:5173/checkout.mp4\n---\n\n# Live\n";
        assert!(demos(source).iter().any(|d| d.code == "demo/remote-fallback"));
    }

    #[test]
    fn a_live_target_is_expected_to_be_remote_and_is_never_reported_for_it() {
        // `live` being remote is what live means. A rule that flagged it would
        // fire on every correctly declared demo in existence.
        let source = "---\ndemo:\n  live: https://app.example.com\n  fallback: ./checkout.mp4\n---\n\n# Live\n";

        let diagnostics = lint_deck(source);
        assert!(
            !diagnostics.iter().any(|d| d.message.contains("app.example.com")),
            "the live target was reported: {diagnostics:?}"
        );
    }

    #[test]
    fn a_slide_declaring_no_demo_is_not_asked_for_a_recording() {
        assert!(demos("# Ordinary\n\nSome content.\n").is_empty());
    }

    #[test]
    fn a_fallback_that_is_only_whitespace_counts_as_missing() {
        let source =
            "---\ndemo:\n  live: https://app.example.com\n  fallback: \"   \"\n---\n\n# Live\n";
        assert!(demos(source).iter().any(|d| d.code == "demo/no-fallback"));
    }
}
