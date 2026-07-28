//! Staging markers: the bridge between prose and the step pipeline.
//!
//! Writing `steps:` frontmatter is precise but heavy for the common case of
//! "reveal these bullets one at a time". Markers are the light form. Both
//! compile to the same [`StepAction`]s, so there is exactly one execution
//! model no matter which the author picked.
//!
//! # The anchor contract
//!
//! A marker compiles to an empty `<span data-slidx-step="N" hidden>` left in
//! the Markdown. It survives any Markdown renderer, which is what keeps slidx
//! framework-agnostic: Vue, React, Svelte, and plain HTML all end up with the
//! same anchor in the same place.
//!
//! At runtime the *staged element* is resolved from the anchor by one rule,
//! implemented identically in the client runtime and the print renderer:
//!
//! 1. If the anchor's parent has no text of its own, the staged element is the
//!    anchor's **previous element sibling**, and the parent is removed. This is
//!    the "marker on its own line" case, which is how whole blocks — code
//!    fences, tables, images — get staged.
//! 2. Otherwise, if the anchor has an `<li>` ancestor, that `<li>` is staged.
//!    This is the "marker at the end of a bullet" case.
//! 3. Otherwise the staged element is the anchor's closest ancestor that is a
//!    direct child of the slide root.
//!
//! Those three cases cover every position a marker can occupy, and each one
//! resolves without inspecting the Markdown renderer's output conventions.

use crate::scanner::{list_item_indent, FenceTracker};
use crate::steps::{AutoSteps, EffectPreset, StepAction, StepOptions};

/// Attribute the runtime queries to find anchors.
pub const ANCHOR_ATTRIBUTE: &str = "data-slidx-step";

/// Markdown body plus the actions its markers produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StagedContent {
    pub content: String,
    pub actions: Vec<StepAction>,
}

/// Builds the selector for an anchor id.
pub fn anchor_selector(id: u32) -> String {
    format!("[{ANCHOR_ATTRIBUTE}=\"{id}\"]")
}

fn anchor_html(id: u32) -> String {
    format!("<span {ANCHOR_ATTRIBUTE}=\"{id}\" hidden></span>")
}

/// Replaces `<!-- step -->` comments with anchors and returns their actions.
///
/// The optional body names a preset: `<!-- step: fly-in -->`. An unrecognised
/// name is ignored rather than rejected, so a typo costs the author an
/// animation, not a slide.
pub fn extract_step_markers(content: &str, next_id: &mut u32) -> StagedContent {
    let mut body = String::with_capacity(content.len());
    let mut actions = Vec::new();
    let mut rest = content;

    while let Some(open_at) = rest.find("<!--") {
        let after_open = &rest[open_at + 4..];
        let Some(close_at) = after_open.find("-->") else {
            break;
        };

        body.push_str(&rest[..open_at]);

        match parse_marker(&after_open[..close_at]) {
            Some(preset) => {
                let id = *next_id;
                *next_id += 1;
                body.push_str(&anchor_html(id));
                actions.push(reveal_anchor(id, preset));
            }
            None => body.push_str(&rest[open_at..open_at + 4 + close_at + 3]),
        }

        rest = &after_open[close_at + 3..];
    }

    body.push_str(rest);
    StagedContent { content: body, actions }
}

fn parse_marker(inner: &str) -> Option<Option<EffectPreset>> {
    let trimmed = inner.trim();
    let remainder = trimmed.strip_prefix("step")?;
    let remainder = remainder.trim_start_matches(':').trim();

    if remainder.is_empty() {
        return Some(None);
    }

    // `<!-- stepper -->` must not match; only a separator may follow `step`.
    if !trimmed[4..].starts_with([':', ' ', '\t']) {
        return None;
    }

    Some(serde_json::from_value(serde_json::Value::String(remainder.to_string())).ok())
}

fn reveal_anchor(id: u32, preset: Option<EffectPreset>) -> StepAction {
    StepAction::Reveal {
        target: anchor_selector(id),
        options: StepOptions { preset, ..StepOptions::default() },
    }
}

/// Appends anchors derived from slide structure.
///
/// Used by `autoSteps`, which stages a slide without the author touching the
/// prose at all — the common case for an agenda or a bullet build.
pub fn inject_auto_steps(content: &str, mode: AutoSteps, next_id: &mut u32) -> StagedContent {
    match mode {
        AutoSteps::List => inject_line_anchors(content, next_id, |line| {
            list_item_indent(line).is_some_and(|indent| indent < 2)
        }),
        AutoSteps::Row => inject_line_anchors(content, next_id, is_table_body_row),
        AutoSteps::Block => inject_block_anchors(content, next_id),
    }
}

/// Appends an anchor to the end of every line the predicate accepts.
fn inject_line_anchors(
    content: &str,
    next_id: &mut u32,
    accept: impl Fn(&str) -> bool,
) -> StagedContent {
    let mut lines = Vec::new();
    let mut actions = Vec::new();
    let mut fences = FenceTracker::new();
    let mut seen_header_rule = false;

    for line in content.lines() {
        if !fences.feed(line) {
            lines.push(line.to_string());
            continue;
        }

        // A table's header separator must precede its body rows.
        if is_table_delimiter(line) {
            seen_header_rule = true;
            lines.push(line.to_string());
            continue;
        }

        let staged = accept(line) && (seen_header_rule || !line.trim_start().starts_with('|'));

        if staged {
            let id = *next_id;
            *next_id += 1;
            lines.push(format!("{line}{}", anchor_html(id)));
            actions.push(reveal_anchor(id, None));
        } else {
            lines.push(line.to_string());
        }
    }

    StagedContent { content: lines.join("\n"), actions }
}

/// Inserts an anchor on its own line after every top-level block.
fn inject_block_anchors(content: &str, next_id: &mut u32) -> StagedContent {
    let mut lines: Vec<String> = Vec::new();
    let mut actions = Vec::new();
    let mut fences = FenceTracker::new();
    let mut in_block = false;

    let flush = |lines: &mut Vec<String>, actions: &mut Vec<StepAction>, next_id: &mut u32| {
        let id = *next_id;
        *next_id += 1;
        lines.push(String::new());
        lines.push(anchor_html(id));
        actions.push(reveal_anchor(id, None));
    };

    for line in content.lines() {
        let is_prose = fences.feed(line);
        let blank = is_prose && line.trim().is_empty();

        if blank && in_block {
            flush(&mut lines, &mut actions, next_id);
            in_block = false;
            lines.push(String::new());
            continue;
        }

        if !blank {
            in_block = true;
        }
        lines.push(line.to_string());
    }

    if in_block {
        flush(&mut lines, &mut actions, next_id);
    }

    StagedContent { content: lines.join("\n"), actions }
}

fn is_table_delimiter(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|')
        && trimmed.contains('-')
        && trimmed.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn is_table_body_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && !is_table_delimiter(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stage(content: &str) -> StagedContent {
        let mut id = 1;
        extract_step_markers(content, &mut id)
    }

    #[test]
    fn a_bare_marker_becomes_an_anchor_and_a_reveal() {
        let result = stage("- one <!-- step -->");

        assert_eq!(result.actions.len(), 1);
        assert_eq!(result.actions[0].targets(), vec!["[data-slidx-step=\"1\"]"]);
        assert!(result.content.contains("data-slidx-step=\"1\""));
        assert!(!result.content.contains("<!-- step -->"));
    }

    #[test]
    fn anchors_are_numbered_across_a_whole_slide() {
        let result = stage("- one <!-- step -->\n- two <!-- step -->");
        assert_eq!(result.actions.len(), 2);
        assert!(result.content.contains("data-slidx-step=\"2\""));
    }

    #[test]
    fn a_marker_can_name_a_preset() {
        let result = stage("- one <!-- step: fly-in -->");
        assert_eq!(result.actions[0].options().preset, Some(EffectPreset::FlyIn));
    }

    #[test]
    fn an_unknown_preset_falls_back_to_the_default_reveal() {
        let result = stage("- one <!-- step: sparkle -->");
        assert_eq!(result.actions.len(), 1, "the step still happens");
        assert_eq!(result.actions[0].options().preset, None);
    }

    #[test]
    fn notes_and_other_comments_are_left_alone() {
        let result = stage("<!-- notes: hi -->\n<!-- stepper -->");
        assert!(result.actions.is_empty());
        assert!(result.content.contains("<!-- notes: hi -->"));
        assert!(result.content.contains("<!-- stepper -->"));
    }

    #[test]
    fn auto_list_stages_only_top_level_items() {
        let mut id = 1;
        let result = inject_auto_steps("- one\n  - nested\n- two", AutoSteps::List, &mut id);

        assert_eq!(result.actions.len(), 2);
        assert!(result.content.contains("- one<span"));
        assert!(result.content.contains("  - nested\n"), "nested items are not separate stops");
    }

    #[test]
    fn auto_list_ignores_list_syntax_inside_code_fences() {
        let mut id = 1;
        let result =
            inject_auto_steps("```md\n- not a step\n```\n\n- real", AutoSteps::List, &mut id);

        assert_eq!(result.actions.len(), 1);
        assert!(result.content.contains("- not a step\n"));
    }

    #[test]
    fn auto_row_stages_table_body_rows_but_not_the_header() {
        let mut id = 1;
        let source = "| a | b |\n| - | - |\n| 1 | 2 |\n| 3 | 4 |";
        let result = inject_auto_steps(source, AutoSteps::Row, &mut id);

        assert_eq!(result.actions.len(), 2, "header and delimiter are not stops");
    }

    #[test]
    fn auto_block_puts_each_anchor_on_its_own_line() {
        let mut id = 1;
        let result = inject_auto_steps("# One\n\nBody.\n", AutoSteps::Block, &mut id);

        assert_eq!(result.actions.len(), 2);
        for line in result.content.lines() {
            if line.contains("data-slidx-step") {
                assert!(line.trim().starts_with("<span"), "block anchors stand alone");
            }
        }
    }

    #[test]
    fn auto_block_keeps_a_fenced_block_whole() {
        let mut id = 1;
        let result = inject_auto_steps(
            "```rust\nlet a = 1;\n\nlet b = 2;\n```\n",
            AutoSteps::Block,
            &mut id,
        );

        assert_eq!(result.actions.len(), 1, "a blank line inside a fence is not a block break");
    }

    #[test]
    fn ids_continue_across_marker_sources() {
        let mut id = 1;
        let inline = extract_step_markers("- one <!-- step -->", &mut id);
        let auto = inject_auto_steps("- two", AutoSteps::List, &mut id);

        assert_eq!(inline.actions[0].targets(), vec!["[data-slidx-step=\"1\"]"]);
        assert_eq!(auto.actions[0].targets(), vec!["[data-slidx-step=\"2\"]"]);
    }

    #[test]
    fn content_without_markers_is_returned_unchanged() {
        let result = stage("# Plain\n\nNo markers here.");
        assert_eq!(result.content, "# Plain\n\nNo markers here.");
        assert!(result.actions.is_empty());
    }
}
