//! A slide's steps, arranged as rows and stops.
//!
//! A timeline surface shows one row per thing a slide addresses and one column
//! per stop. Both of those are decisions about the model rather than about the
//! screen, so they are made here once: which stop an action lands on is a
//! [`compile`](crate::steps::compile) rule, and a second implementation of it in
//! an editor would be a second answer about where a click belongs.
//!
//! What is deliberately *not* here is the cell. A cell is the join of a row and
//! an action that names it, which any caller can do; the rules that a caller
//! could get wrong are the ones this module hands over already decided.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::mark::{find_marks, strip_marks};
use crate::markers::ANCHOR_ATTRIBUTE;
use crate::model::Slide;
use crate::steps::{AutoSteps, StepAction, Visibility};

/// What one authored action does to its targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StepKind {
    Reveal,
    Hide,
    Emphasize,
    Set,
    Group,
}

impl StepKind {
    fn of(action: &StepAction) -> Self {
        match action {
            StepAction::Reveal { .. } => Self::Reveal,
            StepAction::Hide { .. } => Self::Hide,
            StepAction::Emphasize { .. } => Self::Emphasize,
            StepAction::Set { .. } => Self::Set,
            StepAction::Group { .. } => Self::Group,
        }
    }
}

/// One thing a slide's steps can name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StepRow {
    /// The selector a step targets.
    pub target: String,
    /// What to call the row in front of an author.
    pub label: String,
    /// The mark's `#key`, when the author gave this row a name.
    ///
    /// Absent for a row staged by `autoSteps:` or a `<!-- step -->` marker,
    /// which have no name in the source — the reason those rows cannot be
    /// pointed at by hand and the reason a timeline has to show them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub key: Option<String>,
    /// Whether the row is painted at each stop, one entry per stop.
    ///
    /// Read straight off the compiled frames, which is the whole reason a
    /// timeline over this model is cheap: the state at every stop is already
    /// computed, so a caller draws the bar instead of folding the actions
    /// forward. Folding them would be a second copy of the rule about what a
    /// stop means, and the two would eventually disagree about a `hide`.
    pub visible: Vec<bool>,
}

/// One authored action, placed on the grid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StepPlacement {
    /// Position in the slide's action list, which is what an operation names.
    pub index: u32,
    pub kind: StepKind,
    /// The stop this action lands on.
    pub stop: u32,
    /// Every row it touches, group members included.
    pub targets: Vec<String>,
    /// True when it plays on a timer rather than on a press, and therefore
    /// shares the stop before it instead of adding one.
    pub timed: bool,
    /// Canonical `steps:` source, without the leading `- `.
    pub source: String,
}

/// A slide's steps, as a timeline shows them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StepGrid {
    pub rows: Vec<StepRow>,
    pub actions: Vec<StepPlacement>,
    /// Stops on this slide, including the resting frame. Always at least one.
    pub stops: u32,
    /// True when the author wrote `steps:`.
    ///
    /// The one field a timeline must not guess: a generated stop has no line in
    /// the file to change, so a cell is editable in place exactly when this is
    /// true.
    pub declared: bool,
    /// The `autoSteps:` mode in force, whether or not it generated the stops.
    ///
    /// It stays set after the stops are written out, because it is what puts
    /// the anchors in the markup that the written-out steps name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub auto: Option<AutoSteps>,
}

/// How much of a generated row's line is shown before it is cut.
///
/// A row label sits in a fixed column next to a grid of stops, so a long bullet
/// has to end somewhere. Cut rather than wrapped: a row whose height depends on
/// its text would make the grid's lines stop matching its columns.
const LABEL_LIMIT: usize = 48;

/// Projects a slide onto rows and stops.
pub fn step_grid(slide: &Slide) -> StepGrid {
    let mut rows: Vec<StepRow> = Vec::new();

    // Keyed marks first, in the order the author wrote them, because that is
    // the order they read in on the slide. Anything else a step names is
    // appended in the order the steps name it.
    let named = slide.marks.iter().filter_map(|mark| mark.selector().zip(mark.key.clone()));
    for (target, key) in named {
        push_row(&mut rows, target, Some(key), slide);
    }

    let mut actions = Vec::new();
    let mut stop = 0u32;

    for (index, action) in slide.steps.actions.iter().enumerate() {
        // A timed action plays on its own after the stop before it, so it adds
        // no press. The rule is `compile`'s; reading it off `is_auto` here is
        // how the two stay one rule.
        let timed = action.is_auto();
        if !timed {
            stop += 1;
        }

        let targets: Vec<String> = action.targets().into_iter().map(str::to_string).collect();
        for target in &targets {
            push_row(&mut rows, target.clone(), None, slide);
        }

        actions.push(StepPlacement {
            index: index as u32,
            kind: StepKind::of(action),
            stop,
            targets,
            timed,
            source: action.to_source(),
        });
    }

    for row in &mut rows {
        row.visible = slide
            .timeline
            .frames()
            .iter()
            // A row the frames never mention is something the author put on the
            // slide and no step touches, so it is on throughout.
            .map(|frame| frame.visibility(&row.target) != Some(Visibility::Hidden))
            .collect();
    }

    // Marks and generated anchors interleave — a mark can sit inside the bullet
    // that `autoSteps:` staged — so grouping rows by kind would put a row above
    // the line it is part of. Position is what makes the grid read down the
    // slide the way the audience will.
    let where_written = positions(&slide.content);
    rows.sort_by_key(|row| where_written.get(&row.target).copied().unwrap_or(usize::MAX));

    StepGrid {
        rows,
        actions,
        stops: slide.timeline.len() as u32,
        // The parsed frontmatter rather than a flag on the model: `steps:`
        // being written is a fact about the file, and the file is the thing a
        // cell has to have a line in to be editable.
        declared: slide.frontmatter.get("steps").is_some(),
        auto: slide.steps.auto,
    }
}

fn push_row(rows: &mut Vec<StepRow>, target: String, key: Option<String>, slide: &Slide) {
    if rows.iter().any(|row| row.target == target) {
        return;
    }

    let label = key
        .as_deref()
        .and_then(|key| mark_text(slide, key))
        .or_else(|| anchor_text(&slide.content, &target))
        .unwrap_or_else(|| target.clone());

    rows.push(StepRow { target, label, key, visible: Vec::new() });
}

/// Where each addressable target is written in the slide body.
///
/// Only the two kinds that have a place in the text. A hand-written selector
/// names something the renderer produced, which the body does not mention, so it
/// has no position and sorts last.
fn positions(content: &str) -> BTreeMap<String, usize> {
    let mut found = BTreeMap::new();

    for mark in find_marks(content) {
        if let Some(selector) = mark.mark.selector() {
            found.entry(selector).or_insert(mark.start);
        }
    }

    let mut rest = content;
    let mut base = 0usize;
    while let Some(at) = rest.find(&format!("{ANCHOR_ATTRIBUTE}=\"")) {
        let after = &rest[at + ANCHOR_ATTRIBUTE.len() + 2..];
        let Some(close) = after.find('"') else { break };

        found.entry(anchor_selector_of(&after[..close])).or_insert(base + at);
        base += at + ANCHOR_ATTRIBUTE.len() + 2 + close;
        rest = &after[close..];
    }

    found
}

fn anchor_selector_of(id: &str) -> String {
    format!("[{ANCHOR_ATTRIBUTE}=\"{id}\"]")
}

fn mark_text(slide: &Slide, key: &str) -> Option<String> {
    slide.marks.iter().find(|mark| mark.key.as_deref() == Some(key)).map(|mark| mark.text.clone())
}

/// The words an anchored row stages, read out of the slide body.
///
/// A row generated by `autoSteps:` or by a `<!-- step -->` marker has no name
/// in the source, so the only thing an author can be shown is the line it
/// stages. Without it the rows of a generated timeline are indistinguishable
/// from each other, which is the one slide type this surface exists to open up.
fn anchor_text(content: &str, target: &str) -> Option<String> {
    let id = target.strip_prefix(&format!("[{ANCHOR_ATTRIBUTE}=\"")).and_then(|rest| {
        rest.strip_suffix("\"]").filter(|id| id.chars().all(|c| c.is_ascii_digit()))
    })?;

    let anchor = format!("{ANCHOR_ATTRIBUTE}=\"{id}\"");
    let lines: Vec<&str> = content.lines().collect();
    let at = lines.iter().position(|line| line.contains(&anchor))?;

    // An anchor alone on its line stages the block above it — the rule the
    // runtime resolves by — so that is the line the author recognises.
    let own = clean(lines[at]);
    let text = match own.is_empty() {
        false => own,
        true => lines[..at].iter().rev().map(|line| clean(line)).find(|line| !line.is_empty())?,
    };

    Some(shorten(&text))
}

/// One line of Markdown, as the words in it.
///
/// Marks are unwrapped rather than shown: the author picked the phrase by
/// reading it, and `[3.2x faster]{#result}` is the file's spelling of it.
fn clean(line: &str) -> String {
    let text = strip_marks(line);
    let mut out = String::with_capacity(text.len());
    let mut tags = 0usize;

    for character in text.chars() {
        match character {
            '<' => tags += 1,
            '>' if tags > 0 => tags -= 1,
            _ if tags == 0 => out.push(character),
            _ => {}
        }
    }

    out.trim().trim_start_matches(['-', '*', '+', '|', '#', '>']).trim().to_string()
}

fn shorten(text: &str) -> String {
    let mut out: String = text.chars().take(LABEL_LIMIT - 1).collect();

    if out.chars().count() < text.chars().count() {
        out = out.trim_end().to_string();
        out.push('…');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{parse_deck, DeckParseOptions};

    fn grid(source: &str) -> StepGrid {
        let deck = parse_deck(source, &DeckParseOptions::default());
        step_grid(deck.slides.last().expect("a deck always has a slide"))
    }

    #[test]
    fn a_row_is_a_keyed_mark_labelled_with_the_words_it_marks() {
        let grid = grid("# One\n\nThe result was [3.2x faster]{#result .accent}.\n");

        assert_eq!(grid.rows.len(), 1);
        assert_eq!(grid.rows[0].target, "[data-slidx-mark=\"result\"]");
        assert_eq!(grid.rows[0].label, "3.2x faster");
        assert_eq!(grid.rows[0].key.as_deref(), Some("result"));
    }

    #[test]
    fn a_mark_with_no_key_is_not_a_row_because_no_step_could_name_it() {
        let grid = grid("# One\n\nThe result was [3.2x faster]{.accent}.\n");

        assert!(grid.rows.is_empty());
    }

    #[test]
    fn a_row_says_whether_it_is_painted_at_every_stop_rather_than_only_where_it_changes() {
        // A reveal at stop one and a hide at stop three, so the bar the editor
        // draws is off, on, on, off — read off the frames rather than folded
        // out of the actions.
        let source = concat!(
            "---\nsteps:\n  - reveal: \"#it\"\n  - reveal: \".other\"\n  - hide: \"#it\"\n---\n\n",
            "[soon]{#it}\n"
        );
        let row = grid(source).rows.into_iter().find(|row| row.key.is_some()).unwrap();

        assert_eq!(row.visible, [false, true, true, false]);
    }

    #[test]
    fn a_row_that_is_only_emphasised_was_authored_into_the_slide_and_is_on_throughout() {
        let source = "---\nsteps:\n  - emphasize: \"#now\"\n---\n\n[here]{#now}\n";

        assert_eq!(grid(source).rows[0].visible, [true, true]);
        // A mark no step touches is never in a frame, and it is on the slide.
        assert_eq!(grid("# One\n\n[a]{#a}\n").rows[0].visible, [true]);
    }

    #[test]
    fn each_action_reports_the_stop_it_lands_on() {
        let source =
            "---\nsteps:\n  - reveal: \"#a\"\n  - reveal: \"#b\"\n---\n\n[a]{#a} [b]{#b}\n";
        let grid = grid(source);

        // Stop zero is the resting frame, so the first action lands on one.
        assert_eq!(grid.actions.iter().map(|action| action.stop).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(grid.stops, 3);
    }

    #[test]
    fn an_action_on_a_timer_shares_the_stop_before_it_rather_than_adding_one() {
        let source = concat!(
            "---\nsteps:\n",
            "  - reveal: \"#a\"\n",
            "  - reveal: { target: \"#b\", after: 200 }\n",
            "---\n\n[a]{#a} [b]{#b}\n"
        );
        let grid = grid(source);

        assert_eq!(grid.actions.iter().map(|action| action.stop).collect::<Vec<_>>(), [1, 1]);
        assert!(grid.actions[1].timed);
        assert_eq!(grid.stops, 2, "a timed reveal never blocks the presenter");
    }

    #[test]
    fn a_group_reports_every_row_it_touches() {
        let source =
            "---\nsteps:\n  - group: [{ reveal: \"#a\" }, { reveal: \"#b\" }]\n---\n\n[a]{#a} [b]{#b}\n";
        let grid = grid(source);

        assert_eq!(grid.actions.len(), 1);
        assert_eq!(grid.actions[0].kind, StepKind::Group);
        assert_eq!(grid.actions[0].targets.len(), 2);
        assert_eq!(grid.stops, 2, "a group is one press");
    }

    #[test]
    fn an_action_carries_the_line_it_would_be_written_as() {
        let source = "---\nsteps:\n  - hide: \"#a\"\n---\n\n[a]{#a}\n";

        assert_eq!(grid(source).actions[0].source, "hide: \"[data-slidx-mark=\\\"a\\\"]\"");
    }

    #[test]
    fn a_slide_the_author_wrote_steps_for_says_so() {
        let source = "---\nsteps:\n  - reveal: \"#a\"\n---\n\n[a]{#a}\n";

        assert!(grid(source).declared);
    }

    #[test]
    fn stops_generated_from_structure_are_not_declared_and_name_the_mode() {
        let grid = grid("---\nautoSteps: list\n---\n\n- one\n- two\n");

        assert!(!grid.declared);
        assert_eq!(grid.auto, Some(AutoSteps::List));
        assert_eq!(grid.stops, 3);
    }

    #[test]
    fn a_generated_row_is_labelled_with_the_line_it_stages() {
        let grid =
            grid("---\nautoSteps: list\n---\n\n- why the parser matters\n- what it catches\n");

        assert_eq!(grid.rows.len(), 2);
        assert_eq!(grid.rows[0].label, "why the parser matters");
        assert!(grid.rows[0].key.is_none(), "a generated row has no name in the source");
        assert_eq!(grid.rows[0].visible, [false, true, true]);
    }

    #[test]
    fn a_row_staged_by_a_marker_on_its_own_line_is_labelled_with_the_block_above_it() {
        let grid = grid("# One\n\nA whole paragraph.\n\n<!-- step -->\n");

        assert_eq!(grid.rows.len(), 1);
        assert_eq!(grid.rows[0].label, "A whole paragraph.");
    }

    #[test]
    fn a_generated_label_leaves_the_marks_out_because_the_author_reads_the_words() {
        let grid = grid("---\nautoSteps: list\n---\n\n- the result was [3.2x faster]{#result}\n");

        assert_eq!(grid.rows[0].label, "the result was 3.2x faster");
    }

    #[test]
    fn a_long_generated_label_is_cut_rather_than_wrapped() {
        let line = "a".repeat(200);
        let grid = grid(&format!("---\nautoSteps: list\n---\n\n- {line}\n"));

        assert!(grid.rows[0].label.chars().count() <= 48, "{}", grid.rows[0].label);
        assert!(grid.rows[0].label.ends_with('…'));
    }

    #[test]
    fn a_target_no_mark_and_no_anchor_explains_is_still_a_row() {
        // A selector the author wrote by hand. Dropping it would make the
        // timeline disagree with the slide about how many things move.
        let source = "---\nsteps:\n  - reveal: \".chart\"\n---\n\n# One\n";
        let grid = grid(source);

        assert_eq!(grid.rows.len(), 1);
        assert_eq!(grid.rows[0].target, ".chart");
        assert_eq!(grid.rows[0].label, ".chart");
    }

    #[test]
    fn rows_are_listed_in_source_order_and_never_twice() {
        let source = concat!(
            "---\nsteps:\n  - reveal: \"#b\"\n  - reveal: \"#a\"\n  - hide: \"#b\"\n---\n\n",
            "[a]{#a} then [b]{#b}\n"
        );
        let targets: Vec<String> = grid(source).rows.into_iter().map(|row| row.target).collect();

        assert_eq!(targets, ["[data-slidx-mark=\"a\"]", "[data-slidx-mark=\"b\"]"]);
    }

    #[test]
    fn a_mark_inside_a_generated_row_is_listed_under_the_line_it_is_part_of() {
        // Ordered by where each row is written rather than by what kind it is.
        // Grouping marks above anchors would put a phrase above the bullet it
        // belongs to, and the grid would stop reading down the slide.
        let grid = grid("---\nautoSteps: list\n---\n\n- first\n- then [3.2x]{#result}\n");
        let labels: Vec<String> = grid.rows.into_iter().map(|row| row.label).collect();

        assert_eq!(labels, ["first", "then 3.2x", "3.2x"]);
    }

    #[test]
    fn a_slide_with_nothing_staged_is_one_stop_and_no_actions() {
        let grid = grid("# One\n\nBody.\n");

        assert_eq!(grid.stops, 1);
        assert!(grid.actions.is_empty());
        assert!(!grid.declared);
        assert_eq!(grid.auto, None);
    }
}
