//! Markdown source to [`Deck`].
//!
//! Parsing is total: it always produces a renderable deck and reports problems
//! as diagnostics. Every stage is a pure function of the segment it is given,
//! so a slide can be re-parsed on its own when one file changes.

mod segment;

pub use segment::{split, RawFrontmatter, Segment};

use serde_json::Value as JsonValue;

use crate::diagnostic::{Diagnostic, Diagnostics, Severity, SourceSpan};
use crate::frontmatter;
use crate::markers::{extract_step_markers, inject_auto_steps};
use crate::model::{Deck, DeckMeta, Slide};
use crate::notes::extract_notes;
use crate::scanner::{heading_text, FenceTracker};
use crate::slug::{slugify, SlugAllocator};
use crate::steps::{compile_timeline, parse_step_actions, AutoSteps, StepAction, StepSource};

/// How to read a deck source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckParseOptions {
    /// Line that separates slides in a single-file deck.
    pub separator: String,
    /// Applied to slides that do not declare `autoSteps` themselves.
    pub auto_steps: Option<AutoSteps>,
}

impl Default for DeckParseOptions {
    fn default() -> Self {
        Self { separator: "---".to_string(), auto_steps: None }
    }
}

/// Parses a deck source into slides.
pub fn parse_deck(source: &str, options: &DeckParseOptions) -> Deck {
    let mut diagnostics = Diagnostics::default();
    let segments = split(source, &options.separator);

    let mut meta = DeckMeta::default();
    let mut slides = Vec::with_capacity(segments.len());
    let mut slugs = SlugAllocator::new();

    for (index, segment) in segments.iter().enumerate() {
        let matter = read_frontmatter(segment, &mut diagnostics);

        if index == 0 {
            meta = frontmatter::deck_meta(&matter, &mut diagnostics);
        }

        slides.push(build_slide(
            segment,
            matter,
            index as u32,
            options,
            &meta,
            &mut slugs,
            &mut diagnostics,
        ));
    }

    Deck { meta, slides, diagnostics }
}

fn read_frontmatter(segment: &Segment, diagnostics: &mut Diagnostics) -> JsonValue {
    match &segment.frontmatter {
        Some(raw) => frontmatter::parse(&raw.text, raw.line, diagnostics),
        None => frontmatter::empty(),
    }
}

fn build_slide(
    segment: &Segment,
    matter: JsonValue,
    index: u32,
    options: &DeckParseOptions,
    meta: &DeckMeta,
    slugs: &mut SlugAllocator,
    diagnostics: &mut Diagnostics,
) -> Slide {
    let extracted = extract_notes(&segment.body);
    let steps = compile_steps(&extracted.content, &matter, options, index, diagnostics);
    let title = first_heading(&steps.content);

    Slide {
        id: allocate_id(slugs, title.as_deref(), index),
        index,
        title,
        content: steps.content,
        notes: extracted.notes,
        layout: frontmatter::string(&matter, "layout"),
        transition: frontmatter::string(&matter, "transition").or_else(|| meta.transition.clone()),
        budget_seconds: frontmatter::duration_seconds(&matter, "budget"),
        optional: frontmatter::boolean(&matter, "optional").unwrap_or(false),
        timeline: compile_timeline(&steps.source),
        steps: steps.source,
        source_line: segment.line,
        frontmatter: matter,
    }
}

/// The Markdown body plus the pipeline compiled from it.
struct CompiledSteps {
    content: String,
    source: StepSource,
}

/// Resolves a slide's step pipeline from its three possible sources.
///
/// Precedence is explicit rather than merged: an author who writes `steps:`
/// has described the whole slide, and silently appending marker-derived
/// reveals to that list would reorder their pipeline. Markers are still
/// stripped from the body so the ignored ones never reach the audience, and
/// the conflict is reported.
fn compile_steps(
    body: &str,
    matter: &JsonValue,
    options: &DeckParseOptions,
    index: u32,
    diagnostics: &mut Diagnostics,
) -> CompiledSteps {
    let mut next_id = 1u32;
    let staged = extract_step_markers(body, &mut next_id);

    // A slide that mentions `autoSteps` decides for itself, including when it
    // says `none`; only silence falls through to the deck-wide default.
    let auto = frontmatter::auto_steps(matter, diagnostics).unwrap_or(options.auto_steps);
    let (content, auto_actions) = match auto {
        Some(mode) => {
            let injected = inject_auto_steps(&staged.content, mode, &mut next_id);
            (injected.content, injected.actions)
        }
        None => (staged.content, Vec::new()),
    };

    let declared = declared_actions(matter, index, diagnostics);
    let derived: Vec<StepAction> = staged.actions.into_iter().chain(auto_actions).collect();

    let actions = match (declared, derived.is_empty()) {
        (Some(declared), false) => {
            diagnostics.push(
                Diagnostic::new(
                    "steps/markers-ignored",
                    Severity::Info,
                    "`steps:` takes precedence, so step markers on this slide are ignored",
                )
                .at(SourceSpan::default().on_slide(index))
                .with_help("remove `steps:` to use the markers, or fold the markers into it"),
            );
            declared
        }
        (Some(declared), true) => declared,
        (None, _) => derived,
    };

    CompiledSteps { content, source: StepSource { actions, auto } }
}

fn declared_actions(
    matter: &JsonValue,
    index: u32,
    diagnostics: &mut Diagnostics,
) -> Option<Vec<StepAction>> {
    let value = matter.get("steps")?;
    let (actions, errors) = parse_step_actions(value);

    for error in errors {
        diagnostics.push(
            Diagnostic::warning("steps/invalid-action", error)
                .at(SourceSpan::default().on_slide(index)),
        );
    }

    Some(actions)
}

/// The first ATX heading in a slide body, ignoring fenced code.
fn first_heading(body: &str) -> Option<String> {
    let mut fences = FenceTracker::new();

    body.lines()
        .filter(|line| fences.feed(line))
        .find_map(heading_text)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn allocate_id(slugs: &mut SlugAllocator, title: Option<&str>, index: u32) -> String {
    let base = title.map(slugify).filter(|slug| !slug.is_empty());
    slugs.allocate(&base.unwrap_or_else(|| format!("slide-{}", index + 1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Deck {
        parse_deck(source, &DeckParseOptions::default())
    }

    #[test]
    fn a_heading_inside_a_fence_does_not_title_the_slide() {
        let deck = parse("```md\n# Not the title\n```\n\n# Real Title\n");
        assert_eq!(deck.slides[0].title.as_deref(), Some("Real Title"));
    }

    #[test]
    fn a_slide_with_only_a_fence_falls_back_to_an_indexed_id() {
        let deck = parse("```rust\nfn main() {}\n```\n");
        assert_eq!(deck.slides[0].id, "slide-1");
        assert!(deck.slides[0].title.is_none());
    }

    #[test]
    fn budgets_and_optional_flags_are_read_from_frontmatter() {
        let deck = parse("---\nbudget: 90s\noptional: true\n---\n\n# Deep Dive\n");
        assert_eq!(deck.slides[0].budget_seconds, Some(90));
        assert!(deck.slides[0].optional);
    }

    #[test]
    fn slides_inherit_the_deck_transition() {
        let deck = parse("---\ntransition: fade\n---\n\n# One\n\n---\n\n# Two\n");
        assert_eq!(deck.slides[1].transition.as_deref(), Some("fade"));
    }

    #[test]
    fn declared_steps_win_over_markers_and_the_conflict_is_reported() {
        let deck = parse("---\nsteps:\n  - reveal: \".x\"\n---\n\n- one <!-- step -->\n");

        assert_eq!(deck.slides[0].steps.actions.len(), 1);
        assert_eq!(deck.slides[0].steps.actions[0].targets(), vec![".x"]);
        assert!(deck.diagnostics.iter().any(|d| d.code == "steps/markers-ignored"));
        assert!(!deck.slides[0].content.contains("<!-- step -->"), "the marker never ships");
    }

    #[test]
    fn an_invalid_declared_action_warns_without_dropping_the_slide() {
        let deck = parse("---\nsteps:\n  - teleport: \".x\"\n---\n\n# One\n");

        assert_eq!(deck.slides.len(), 1);
        assert!(deck.diagnostics.iter().any(|d| d.code == "steps/invalid-action"));
        assert!(!deck.diagnostics.has_blocking());
    }

    #[test]
    fn anchor_numbering_restarts_on_every_slide() {
        // Ids are scoped to a slide so editing one slide never renumbers the
        // next, which keeps incremental rebuilds and diffs small.
        let deck = parse("- a <!-- step -->\n\n---\n\n- b <!-- step -->\n");

        assert!(deck.slides[0].content.contains("data-slidx-step=\"1\""));
        assert!(deck.slides[1].content.contains("data-slidx-step=\"1\""));
    }

    #[test]
    fn a_deck_wide_auto_steps_option_applies_to_slides_that_do_not_opt_out() {
        let options = DeckParseOptions { auto_steps: Some(AutoSteps::List), ..Default::default() };
        let deck = parse_deck("- one\n- two\n", &options);
        assert_eq!(deck.slides[0].timeline.len(), 3);
    }

    #[test]
    fn a_slide_can_opt_out_of_a_deck_wide_auto_steps_option() {
        let options = DeckParseOptions { auto_steps: Some(AutoSteps::List), ..Default::default() };
        let deck = parse_deck("---\nautoSteps: none\n---\n\n- one\n- two\n", &options);
        assert_eq!(deck.slides[0].timeline.len(), 1);
    }

    #[test]
    fn indexes_and_source_lines_are_assigned_in_order() {
        let deck = parse("# One\n\n---\n\n# Two\n\n---\n\n# Three\n");

        for (position, slide) in deck.slides.iter().enumerate() {
            assert_eq!(slide.index as usize, position);
        }
        assert!(deck.slides[1].source_line > deck.slides[0].source_line);
    }

    #[test]
    fn an_empty_source_produces_one_empty_slide() {
        let deck = parse("");
        assert_eq!(deck.slides.len(), 1);
        assert_eq!(deck.slides[0].id, "slide-1");
    }

    #[test]
    fn notes_are_removed_before_step_markers_are_resolved() {
        // A marker mentioned inside a note must not become a real step.
        let deck = parse("# One\n\n<!-- notes: mention the <!-- step --> trick -->\n");
        assert!(deck.slides[0].timeline.is_single_stop());
    }
}
