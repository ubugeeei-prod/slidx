//! Markdown source to [`Deck`].
//!
//! Parsing is total: it always produces a renderable deck and reports problems
//! as diagnostics. Every stage is a pure function of the segment it is given,
//! so a slide can be re-parsed on its own when one file changes.

mod segment;

pub use segment::{split, RawFrontmatter, Segment};

use serde_json::Value as JsonValue;

use crate::block::extract_blocks;
use crate::diagnostic::{Diagnostic, Diagnostics, Severity, SourceSpan};
use crate::frontmatter;
use crate::mark::{compile_marks, find_marks, stage_takes, strip_marks, Mark};
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

    // Frontmatter is read for the whole deck before any slide is built, because
    // pinned ids have to be reserved before the first slug is allocated. A slide
    // pins the id a published deck already addresses it by, and a slug that
    // collided with one has to be the thing that moves.
    let matters: Vec<JsonValue> =
        segments.iter().map(|segment| read_frontmatter(segment, &mut diagnostics)).collect();

    let meta = match matters.first() {
        Some(matter) => frontmatter::deck_meta(matter, &mut diagnostics),
        None => DeckMeta::default(),
    };

    let mut slugs = SlugAllocator::new();
    for (index, matter) in matters.iter().enumerate() {
        let Some(id) = pinned_id(matter) else { continue };

        if !slugs.reserve(&id) {
            diagnostics.push(
                Diagnostic::warning("deck/duplicate-id", format!("`id: {id}` is pinned twice"))
                    .at(SourceSpan::default().on_slide(index as u32))
                    .with_help("one id addresses one slide; give this one an id of its own"),
            );
        }
    }

    let slides = segments
        .iter()
        .zip(matters)
        .enumerate()
        .map(|(index, (segment, matter))| {
            build_slide(segment, matter, index as u32, options, &meta, &mut slugs, &mut diagnostics)
        })
        .collect();

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

    // The title is the slide's words without its styling: `Making [decks]{.accent}
    // fast` is titled "Making decks fast" everywhere it is quoted — the outline,
    // the OG image, the published description, and the PDF bookmark.
    let title = first_heading(&strip_marks(&steps.content));

    let marks: Vec<Mark> = find_marks(&steps.content).into_iter().map(|found| found.mark).collect();
    let mut next_key = 1u32;

    // Blocks are taken last, from the content that will actually be rendered.
    // Anything earlier would record spans that the anchors and marks compiled
    // after it then shift out from under.
    let blocked = extract_blocks(&compile_marks(&steps.content, &mut next_key));

    Slide {
        id: allocate_id(slugs, &matter, title.as_deref(), index),
        index,
        title,
        content: blocked.content,
        blocks: blocked.blocks,
        marks,
        notes: extracted.notes,
        layout: frontmatter::string(&matter, "layout"),
        transition: slide_transition(&matter, index, meta, diagnostics),
        budget_seconds: frontmatter::duration_seconds(&matter, "budget"),
        optional: frontmatter::boolean(&matter, "optional").unwrap_or(false),
        demo: crate::demo::parse(&matter),
        timeline: compile_timeline(&steps.source),
        steps: steps.source,
        source_line: segment.line,
        frontmatter: matter,
    }
}

/// The transition one slide arrives with.
///
/// A slide that names one decides for itself, including when it says `none`;
/// only silence inherits the deck's. The first slide is skipped because its
/// frontmatter *is* the deck's — reading it again would report the same
/// malformed value twice, at the same line.
fn slide_transition(
    matter: &JsonValue,
    index: u32,
    meta: &DeckMeta,
    diagnostics: &mut Diagnostics,
) -> Option<String> {
    if index == 0 {
        return meta.transition.clone();
    }

    frontmatter::transition(matter, diagnostics).or_else(|| meta.transition.clone())
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

    // Takes are lifted last, so their `Set` steps land after the reveals that
    // brought the elements on screen in the first place. A slide that needs a
    // different order should say so with an explicit `steps:` list.
    let takes = stage_takes(&content);
    let content = takes.content;

    for key in &takes.ambiguous_keys {
        diagnostics.push(
            Diagnostic::warning(
                "mark/ambiguous-key",
                format!("`#{key}` is used more than once but the marks are not adjacent"),
            )
            .at(SourceSpan::default().on_slide(index))
            .with_help(
                "put takes next to each other to mean one changing element,                  or give them different keys",
            ),
        );
    }

    let declared = declared_actions(matter, index, diagnostics);
    // `autoSteps:` alongside `steps:` is not a conflict, and this is the one
    // place that has to know it: the mode injects the anchors an explicit list
    // targets, which is exactly what the editor's timeline writes when an author
    // asks to edit generated stops. Only staging written into the body — a
    // marker, a take — actually loses its meaning to a declared list.
    let authored = !staged.actions.is_empty() || !takes.actions.is_empty();
    let derived: Vec<StepAction> =
        staged.actions.into_iter().chain(auto_actions).chain(takes.actions).collect();

    let actions = match (declared, authored) {
        (Some(declared), true) => {
            diagnostics.push(
                Diagnostic::new(
                    "steps/markers-ignored",
                    Severity::Info,
                    "`steps:` takes precedence, so the staging written into this slide is ignored",
                )
                .at(SourceSpan::default().on_slide(index))
                .with_help("remove `steps:` to use the markers, or fold the markers into it"),
            );
            declared
        }
        (Some(declared), false) => declared,
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

/// The id a slide declares for itself, if it declares one.
///
/// A slide id is a slug of its heading, which means translating the heading
/// moves the slide: every deep link into the deck and every QR code printed on
/// a handout addresses the old one. `id:` is how a deck keeps an address it has
/// already published while its words change underneath.
fn pinned_id(matter: &JsonValue) -> Option<String> {
    frontmatter::string(matter, "id").map(|id| id.trim().to_string()).filter(|id| !id.is_empty())
}

fn allocate_id(
    slugs: &mut SlugAllocator,
    matter: &JsonValue,
    title: Option<&str>,
    index: u32,
) -> String {
    // Returned rather than allocated: the pin was reserved before any slug was,
    // so passing it through the allocator now would suffix it against itself.
    if let Some(pinned) = pinned_id(matter) {
        return pinned;
    }

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
    fn a_slide_can_pin_the_id_its_heading_would_otherwise_derive() {
        // What makes a translated deck reachable at the URLs the original one
        // published. Without this, translating a heading moves the slide.
        let deck = parse("---\nid: getting-started\n---\n\n## はじめに\n");

        assert_eq!(deck.slides[0].id, "getting-started");
    }

    #[test]
    fn a_pinned_id_wins_a_collision_against_a_heading_that_now_slugs_to_it() {
        // Pins are the addresses a deck already published, so a heading that
        // happens to slug onto one has to move rather than the pin. The other
        // way round is a broken deep link nothing reports.
        let deck = parse("# Overview\n\n---\nid: overview\n---\n\n# Something Else\n");

        assert_eq!(deck.slides[1].id, "overview");
        assert_eq!(deck.slides[0].id, "overview-2");
    }

    #[test]
    fn two_slides_pinning_one_id_is_reported_rather_than_silently_merged() {
        let deck = parse("---\nid: intro\n---\n\n# One\n\n---\nid: intro\n---\n\n# Two\n");

        assert!(deck.diagnostics.iter().any(|d| d.code == "deck/duplicate-id"));
    }

    #[test]
    fn a_deck_says_which_language_it_is_in_and_what_it_is_a_translation_of() {
        let deck = parse("---\nlang: ja\ntranslationOf: ../slides\n---\n\n# はじめに\n");

        assert_eq!(deck.meta.lang.as_deref(), Some("ja"));
        assert_eq!(deck.meta.translation_of.as_deref(), Some("../slides"));
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
    fn a_slide_carries_the_demo_it_declares() {
        let deck = parse(
            "---\ndemo:\n  live: https://app.example.com\n  fallback: ./checkout.mp4\n---\n\n# Live\n",
        );

        let demo = deck.slides[0].demo.as_ref().unwrap();
        assert_eq!(demo.live, "https://app.example.com");
        assert_eq!(demo.fallback.as_deref(), Some("./checkout.mp4"));
    }

    #[test]
    fn a_slide_without_a_demo_carries_none() {
        assert!(parse("# Ordinary\n").slides[0].demo.is_none());
    }

    #[test]
    fn slides_inherit_the_deck_transition() {
        let deck = parse("---\ntransition: fade\n---\n\n# One\n\n---\n\n# Two\n");
        assert_eq!(deck.slides[1].transition.as_deref(), Some("fade"));
    }

    #[test]
    fn a_slide_can_override_the_deck_transition() {
        let deck =
            parse("---\ntransition: fade\n---\n\n# One\n\n---\ntransition: push\n---\n\n# Two\n");

        assert_eq!(deck.slides[0].transition.as_deref(), Some("fade"));
        assert_eq!(deck.slides[1].transition.as_deref(), Some("push"));
    }

    #[test]
    fn a_slide_can_switch_off_a_deck_wide_transition() {
        // A slide that has to land without motion — a demo, a video — needs
        // `none` to win over the deck default rather than fall through to it.
        let deck =
            parse("---\ntransition: push\n---\n\n# One\n\n---\ntransition: false\n---\n\n# Two\n");

        assert_eq!(deck.slides[1].transition.as_deref(), Some("none"));
    }

    #[test]
    fn a_malformed_deck_transition_is_reported_once_not_once_per_reader() {
        // The first slide's frontmatter is also the deck's, and reading it
        // twice would print the same complaint twice at the same line.
        let deck = parse("---\ntransition: 3\n---\n\n# One\n");

        assert_eq!(
            deck.diagnostics.iter().filter(|d| d.code == "frontmatter/invalid-transition").count(),
            1
        );
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
    fn auto_steps_beside_declared_steps_is_not_reported_because_it_supplies_the_anchors() {
        // What the editor's timeline writes when an author asks to edit the
        // stops `autoSteps:` generated. The mode has to stay — it is what puts
        // `[data-slidx-step="N"]` in the markup — so a slide in that shape is
        // correct rather than in conflict with itself.
        let deck = parse("---\nautoSteps: list\nsteps:\n  - reveal: \".x\"\n---\n\n- one\n");

        assert_eq!(deck.slides[0].steps.actions.len(), 1);
        assert!(deck.slides[0].content.contains("data-slidx-step=\"1\""));
        assert!(!deck.diagnostics.iter().any(|d| d.code == "steps/markers-ignored"));
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
