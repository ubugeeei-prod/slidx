//! Turning JSON arguments into slidx's own types.
//!
//! One module because the conversions are shared and because every one of them
//! is a place a model can be wrong. What matters is the *refusal*: a schema
//! violation is caught by the client, so anything that reaches here is
//! well-formed JSON that still does not name something real, and the answer has
//! to say what would have.
//!
//! ## Why a mark is named by its text
//!
//! [`slidx_edit::EditOp::AddMark`] takes a byte range in the slide's body,
//! because that is what a text selection in a canvas maps to. A model has no
//! selection and counting bytes through a paragraph of Japanese is exactly the
//! sort of arithmetic it gets wrong. So a tool takes the text to wrap and this
//! module finds it — and refuses when the text appears more than once without
//! being told which, rather than marking the first and hoping.

use serde_json::Value;

use slidx_core::{ByteSpan, StepAction};
use slidx_edit::{MarkAttributes, MarkRef, SlideRef};

use crate::mcp::workspace::Reading;

/// A required string argument.
pub fn required(arguments: &Value, key: &str, what: &str) -> Result<String, String> {
    text(arguments, key).map(str::to_string).ok_or_else(|| format!("`{key}` is required: {what}"))
}

/// An optional string argument, treating an empty one as absent.
pub fn text<'a>(arguments: &'a Value, key: &str) -> Option<&'a str> {
    arguments.get(key).and_then(Value::as_str).filter(|value| !value.is_empty())
}

/// A string argument that may be empty, which is how notes are removed.
pub fn string(arguments: &Value, key: &str) -> Option<String> {
    arguments.get(key).and_then(Value::as_str).map(str::to_string)
}

pub fn number(arguments: &Value, key: &str) -> Option<usize> {
    arguments.get(key).and_then(Value::as_u64).map(|value| value as usize)
}

pub fn strings(arguments: &Value, key: &str) -> Vec<String> {
    arguments
        .get(key)
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

/// Which slide, by zero-based index or by slug.
pub fn slide(arguments: &Value) -> Result<SlideRef, String> {
    match arguments.get("slide") {
        Some(Value::Number(index)) => index
            .as_u64()
            .map(|index| SlideRef::Index(index as usize))
            .ok_or_else(|| "`slide` counts from zero, so it cannot be negative.".to_string()),
        Some(Value::String(id)) => Ok(SlideRef::Id(id.clone())),
        _ => Err("`slide` is required: the slide's zero-based index, or its slug.".to_string()),
    }
}

/// Which mark, by position in source order or by its `#key`.
///
/// A leading `#` is accepted and stripped, because that is how a key is written
/// in a slide and in a step target, and a model that copied it from either would
/// otherwise name a mark whose key begins with a hash.
pub fn mark(arguments: &Value) -> Result<MarkRef, String> {
    match arguments.get("mark") {
        Some(Value::Number(index)) => index
            .as_u64()
            .map(|index| MarkRef::Index(index as usize))
            .ok_or_else(|| "`mark` counts from zero, so it cannot be negative.".to_string()),
        Some(Value::String(key)) => {
            Ok(MarkRef::Key(key.strip_prefix('#').unwrap_or(key).to_string()))
        }
        _ => {
            Err("`mark` is required: the mark's position in the slide, or its `#key`.".to_string())
        }
    }
}

/// A mark's attributes.
///
/// Empty attributes are not an error: they are how a mark is unwrapped back to
/// plain text, which is what `[text]{}` would have meant if anybody wrote it.
pub fn attributes(arguments: &Value) -> MarkAttributes {
    let properties = arguments
        .get("properties")
        .and_then(Value::as_object)
        .map(|table| {
            table
                .iter()
                .filter_map(|(name, value)| {
                    value.as_str().map(|value| (name.clone(), value.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();

    MarkAttributes {
        key: text(arguments, "key").map(|key| key.strip_prefix('#').unwrap_or(key).to_string()),
        classes: strings(arguments, "classes")
            .into_iter()
            .map(|class| class.strip_prefix('.').unwrap_or(&class).to_string())
            .collect(),
        properties,
    }
}

/// The byte range of the text a mark should wrap, inside the slide's body.
///
/// Refuses rather than guessing when the text appears more than once and the
/// caller did not say which. Marking the wrong three words is worse than not
/// marking them, because nothing about the file says which was meant.
pub fn range(reading: &Reading, slide: &SlideRef, arguments: &Value) -> Result<ByteSpan, String> {
    let wanted = required(arguments, "text", "the exact text the mark should wrap.")?;
    let body = body_of(reading, slide)?;

    let found: Vec<usize> = body.match_indices(&wanted).map(|(at, _)| at).collect();

    let at = match (found.len(), number(arguments, "occurrence")) {
        (0, _) => {
            return Err(format!(
                "The slide's text does not contain {wanted:?}. It reads:\n\n{body}"
            ))
        }
        (_, Some(occurrence)) => *found.get(occurrence.saturating_sub(1)).ok_or_else(|| {
            format!(
                "{wanted:?} appears {} time(s) on the slide, so there is no occurrence {occurrence}.",
                found.len()
            )
        })?,
        (1, None) => found[0],
        (count, None) => {
            return Err(format!(
                "{wanted:?} appears {count} times on the slide, so it is not clear which one to \
                 mark. Pass `occurrence` — 1 for the first — or give more surrounding text."
            ))
        }
    };

    Ok(ByteSpan::new(at, at + wanted.len()))
}

/// The Markdown body of one slide, as a mark's range is measured in it.
fn body_of(reading: &Reading, slide: &SlideRef) -> Result<String, String> {
    let options = slidx_core::DeckParseOptions {
        separator: reading.separator.clone(),
        ..slidx_core::DeckParseOptions::default()
    };
    let spans = slidx_edit::slide_spans(&reading.source, &options);

    let index = match slide {
        SlideRef::Index(index) => *index,
        SlideRef::Id(id) => reading
            .deck
            .slides
            .iter()
            .position(|slide| slide.id == *id)
            .ok_or_else(|| format!("There is no slide with the id `{id}`."))?,
    };

    spans
        .get(index)
        .map(|span| span.body.slice(&reading.source).to_string())
        .ok_or_else(|| format!("There is no slide at index {index}."))
}

/// One step action, from the shape a tool offers rather than serde's.
///
/// `StepAction` is an externally tagged enum, so its own serialisation is
/// `{"reveal": {"target": …, "options": …}}` — a shape a model gets wrong and
/// which buries the tuning it rarely wants inside the thing it always does. This
/// takes a flat object instead and builds the action with slidx's own
/// constructors, so the defaults are slidx's.
pub fn action(arguments: &Value) -> Result<StepAction, String> {
    let kind = required(arguments, "action", "one of reveal, hide, emphasize, set, or group.")?;

    let built = match kind.as_str() {
        "reveal" => StepAction::reveal(target(arguments)?),
        "hide" => StepAction::hide(target(arguments)?),
        "emphasize" => StepAction::Emphasize {
            target: target(arguments)?,
            options: slidx_core::StepOptions::default(),
        },
        "set" => StepAction::set(target(arguments)?, patch(arguments)?),
        "group" => {
            let actions: Vec<StepAction> = arguments
                .get("actions")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    "A group needs `actions`: the intents that land on the same click.".to_string()
                })?
                .iter()
                .map(action)
                .collect::<Result<_, _>>()?;

            if actions.is_empty() {
                return Err("A group of no actions is not a step.".to_string());
            }

            StepAction::group(actions)
        }
        other => {
            return Err(format!(
                "`{other}` is not a step action. It is one of reveal, hide, emphasize, set, or \
                 group — and remember that a value which *changes* is two takes of one mark \
                 rather than a hide and a reveal."
            ))
        }
    };

    Ok(tuned(built, arguments))
}

/// Applies the timing and animation a caller asked for.
///
/// Anything left out stays at slidx's default and is not written into the
/// deck — an editor that spells out every key turns a one-word change into a
/// diff across the whole slide.
fn tuned(action: StepAction, arguments: &Value) -> StepAction {
    let mut action = action;

    if let Some(preset) = text(arguments, "preset").and_then(preset) {
        action = action.with_preset(preset);
    }
    if let Some(duration) = number(arguments, "durationMs") {
        action = action.with_duration(duration as u32);
    }
    if let Some(after) = number(arguments, "afterMs") {
        action = action.after_ms(after as u32);
    }

    action
}

/// A preset by the token slidx writes into a deck.
///
/// Matched against `EffectPreset::ALL` rather than a list here, so a preset added
/// to `slidx_core` is accepted without anybody remembering to add it — the same
/// enum the tool's schema enumerates.
fn preset(token: &str) -> Option<slidx_core::EffectPreset> {
    slidx_core::EffectPreset::ALL.into_iter().find(|preset| preset.as_token() == token)
}

fn target(arguments: &Value) -> Result<String, String> {
    required(arguments, "target", "the mark or element the step acts on, such as `#latency`.")
}

fn patch(arguments: &Value) -> Result<slidx_core::steps::action::Patch, String> {
    let mut patch = slidx_core::steps::action::Patch::default();

    if let Some(content) = string(arguments, "content") {
        patch.content = Some(content);
    }

    if let Some(table) = arguments.get("properties").and_then(Value::as_object) {
        for (name, value) in table {
            if let Some(value) = value.as_str() {
                patch.properties.insert(name.clone(), value.to_string());
            }
        }
    }

    if patch.is_empty() {
        return Err(
            "A `set` step needs `content`, `properties`, or both — otherwise it changes nothing."
                .to_string(),
        );
    }

    Ok(patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A reading over a source, without touching a filesystem.
    fn reading(source: &str) -> Reading {
        let options = slidx_core::DeckParseOptions::default();

        Reading {
            path: std::path::PathBuf::from("/deck/slides"),
            label: "slides".into(),
            deck: slidx_core::parse_deck(source, &options),
            source: source.to_string(),
            files: Vec::new(),
            separator: "---".into(),
        }
    }

    #[test]
    fn a_slide_is_named_by_a_bare_number_or_a_bare_string() {
        assert_eq!(slide(&json!({ "slide": 2 })).expect("a slide"), SlideRef::Index(2));
        assert_eq!(
            slide(&json!({ "slide": "intro" })).expect("a slide"),
            SlideRef::Id("intro".into())
        );
    }

    #[test]
    fn a_missing_slide_says_both_ways_of_naming_one() {
        let refusal = slide(&json!({})).expect_err("no slide");

        assert!(refusal.contains("zero-based index"), "{refusal}");
        assert!(refusal.contains("slug"), "{refusal}");
    }

    #[test]
    fn a_key_copied_with_its_hash_still_names_the_mark() {
        // `#latency` is how a key is written in a slide and in a step target, so
        // it is what a model copies.
        assert_eq!(
            mark(&json!({ "mark": "#latency" })).expect("a mark"),
            MarkRef::Key("latency".into())
        );
        assert_eq!(attributes(&json!({ "key": "#hero" })).key.as_deref(), Some("hero"));
    }

    #[test]
    fn a_class_written_with_its_dot_is_still_one_class() {
        assert_eq!(
            attributes(&json!({ "classes": [".accent", "big"] })).classes,
            ["accent", "big"]
        );
    }

    #[test]
    fn empty_attributes_are_how_a_mark_is_unwrapped_rather_than_an_error() {
        assert!(attributes(&json!({})).is_empty());
    }

    #[test]
    fn the_text_a_mark_should_wrap_is_found_in_the_slides_body() {
        // The range is measured in the slide's body, which is what a selection
        // in the visual editor maps to and what `EditOp::AddMark` expects.
        let body = "# One\n\nThe result was 3.2x faster.";
        let deck = reading("# One\n\nThe result was 3.2x faster.\n");
        let span =
            range(&deck, &SlideRef::Index(0), &json!({ "text": "3.2x faster" })).expect("a range");

        assert_eq!(span.slice(body), "3.2x faster");
    }

    #[test]
    fn text_that_appears_twice_is_refused_rather_than_marked_at_a_guess() {
        // Marking the wrong three words is worse than not marking them: nothing
        // about the file afterwards says which was meant.
        let deck = reading("# One\n\nfast, then fast again\n");
        let refusal =
            range(&deck, &SlideRef::Index(0), &json!({ "text": "fast" })).expect_err("ambiguous");

        assert!(refusal.contains("appears 2 times"), "{refusal}");
        assert!(refusal.contains("occurrence"), "{refusal}");
    }

    #[test]
    fn an_occurrence_picks_which_one() {
        let body = "# One\n\nfast, then fast again";
        let deck = reading("# One\n\nfast, then fast again\n");
        let span = range(&deck, &SlideRef::Index(0), &json!({ "text": "fast", "occurrence": 2 }))
            .expect("a range");

        assert_eq!(span.start, body.rfind("fast").expect("the second one"));
    }

    #[test]
    fn text_that_is_not_there_is_answered_with_what_the_slide_does_say() {
        // So the next attempt can be right rather than another guess.
        let deck = reading("# One\n\nThe result was 3.2x faster.\n");
        let refusal = range(&deck, &SlideRef::Index(0), &json!({ "text": "nowhere" }))
            .expect_err("not there");

        assert!(refusal.contains("3.2x faster"), "{refusal}");
    }

    #[test]
    fn a_range_is_measured_in_bytes_so_japanese_lands_where_it_was_meant() {
        // Three bytes per kanji. A character count would cut one in half.
        let deck = reading("# 導入\n\n速度が上がりました。\n");
        let span = range(&deck, &SlideRef::Index(0), &json!({ "text": "速度" })).expect("a range");

        assert_eq!(span.len(), "速度".len());
        assert_eq!(span.len(), 6);
    }

    #[test]
    fn a_step_is_built_from_a_flat_object_rather_than_serdes_tagged_shape() {
        // `{"reveal": {"target": …, "options": …}}` is a shape a model gets
        // wrong, and it buries the tuning inside the thing it always does.
        let built = action(&json!({ "action": "reveal", "target": "#result" })).expect("an action");

        assert_eq!(built, StepAction::reveal("#result"));
    }

    #[test]
    fn tuning_left_out_stays_at_slidxs_own_default() {
        // An editor that spells out every key turns a one-word change into a
        // diff across the whole slide.
        let built = action(&json!({ "action": "hide", "target": "#a" })).expect("an action");
        let fields = match &built {
            StepAction::Hide { options, .. } => options.to_fields(),
            other => panic!("expected a hide, got {other:?}"),
        };

        assert!(fields.is_empty(), "{fields:?}");
    }

    #[test]
    fn tuning_that_was_asked_for_is_applied() {
        let built = action(
            &json!({ "action": "reveal", "target": "#a", "preset": "fly-in", "afterMs": 250 }),
        )
        .expect("an action");
        let fields = match &built {
            StepAction::Reveal { options, .. } => options.to_fields(),
            other => panic!("expected a reveal, got {other:?}"),
        };

        assert!(fields.contains(&("preset".to_string(), "fly-in".to_string())), "{fields:?}");
        assert!(fields.contains(&("after".to_string(), "250".to_string())), "{fields:?}");
    }

    #[test]
    fn a_group_lands_several_intents_on_one_click() {
        let built = action(&json!({
            "action": "group",
            "actions": [
                { "action": "reveal", "target": "#a" },
                { "action": "hide", "target": "#b" },
            ],
        }))
        .expect("a group");

        match built {
            StepAction::Group { actions, .. } => assert_eq!(actions.len(), 2),
            other => panic!("expected a group, got {other:?}"),
        }
    }

    #[test]
    fn a_set_step_that_changes_nothing_is_refused() {
        let refusal = action(&json!({ "action": "set", "target": "#a" })).expect_err("empty patch");

        assert!(refusal.contains("changes nothing"), "{refusal}");
    }

    #[test]
    fn an_unknown_action_names_the_ones_there_are_and_warns_about_takes() {
        // The mistake this catches: reaching for a hide and a reveal when the
        // gesture is a value that changes, which is two takes of one mark.
        let refusal =
            action(&json!({ "action": "replace", "target": "#a" })).expect_err("no such action");

        assert!(refusal.contains("takes"), "{refusal}");
    }
}
