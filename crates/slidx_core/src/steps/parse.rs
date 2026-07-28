//! Reading the `steps:` frontmatter list.
//!
//! Split from the action model because they answer different questions: the
//! model says what a step *is*, this says how to recognise one in what an
//! author wrote. It is also the only part that has to be forgiving — a typo in
//! one animation must cost that animation, never the deck someone is about to
//! present, so every failure here is collected and reported rather than
//! returned as an error.

use serde_json::Value as JsonValue;

use super::action::{StepAction, StepOptions};

/// Parses the `steps:` frontmatter list.
///
/// Unknown or malformed entries are skipped and reported, because a typo in one
/// animation should never take down the deck someone is about to present.
pub fn parse_step_actions(value: &JsonValue) -> (Vec<StepAction>, Vec<String>) {
    let mut actions = Vec::new();
    let mut errors = Vec::new();

    let Some(items) = value.as_array() else {
        errors.push("`steps` must be a list of actions".to_string());
        return (actions, errors);
    };

    for (index, item) in items.iter().enumerate() {
        match parse_step_action(item) {
            Ok(action) => actions.push(action),
            Err(message) => errors.push(format!("steps[{index}]: {message}")),
        }
    }

    (actions, errors)
}

fn parse_step_action(value: &JsonValue) -> Result<StepAction, String> {
    let object =
        value.as_object().ok_or("each step must be a mapping such as `- reveal: \".a\"`")?;
    if object.len() != 1 {
        return Err("each step must name exactly one action".to_string());
    }

    let (name, body) = object.iter().next().expect("length checked above");

    match name.as_str() {
        "reveal" => build_target_action(body, StepKindTag::Reveal),
        "hide" => build_target_action(body, StepKindTag::Hide),
        "emphasize" | "emphasise" => build_target_action(body, StepKindTag::Emphasize),
        "group" => {
            let (nested, errors) = parse_step_actions(group_body(body));
            if !errors.is_empty() {
                return Err(errors.join("; "));
            }
            Ok(StepAction::Group { actions: nested, options: parse_options(body) })
        }
        other => Err(format!("unknown action `{other}`")),
    }
}

/// A `group` accepts either a bare list or `{ actions: [...] }` with options.
fn group_body(body: &JsonValue) -> &JsonValue {
    body.get("actions").unwrap_or(body)
}

enum StepKindTag {
    Reveal,
    Hide,
    Emphasize,
}

fn build_target_action(body: &JsonValue, tag: StepKindTag) -> Result<StepAction, String> {
    let target = match body {
        JsonValue::String(target) => target.clone(),
        JsonValue::Object(_) => {
            body.get("target").and_then(JsonValue::as_str).ok_or("missing `target`")?.to_string()
        }
        _ => return Err("expected a selector string or a mapping with `target`".to_string()),
    };

    if target.trim().is_empty() {
        return Err("`target` must not be empty".to_string());
    }

    let options = parse_options(body);
    Ok(match tag {
        StepKindTag::Reveal => StepAction::Reveal { target, options },
        StepKindTag::Hide => StepAction::Hide { target, options },
        StepKindTag::Emphasize => StepAction::Emphasize { target, options },
    })
}

fn parse_options(body: &JsonValue) -> StepOptions {
    let mut options = StepOptions::default();
    let Some(object) = body.as_object() else {
        return options;
    };

    if let Some(after) = object.get("after").and_then(JsonValue::as_u64) {
        options.after = Some(after as u32);
    }
    if let Some(duration) = object.get("duration").and_then(JsonValue::as_u64) {
        options.duration = duration as u32;
    }
    if let Some(preset) = object.get("preset") {
        options.preset = serde_json::from_value(preset.clone()).ok();
    }
    if let Some(easing) = object.get("easing") {
        if let Ok(parsed) = serde_json::from_value(easing.clone()) {
            options.easing = parsed;
        }
    }
    if let Some(origin) = object.get("origin").or_else(|| object.get("from")) {
        options.origin = serde_json::from_value(origin.clone()).ok();
    }

    options
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steps::preset::{EffectKind, EffectPreset, Origin};
    use serde_json::json;

    #[test]
    fn a_bare_string_body_is_the_target() {
        let (actions, errors) = parse_step_actions(&json!([{ "reveal": ".a" }]));
        assert!(errors.is_empty());
        assert_eq!(actions, vec![StepAction::reveal(".a")]);
    }

    #[test]
    fn a_mapping_body_carries_options() {
        let (actions, errors) = parse_step_actions(&json!([
            { "reveal": { "target": ".a", "preset": "fly-in", "from": "left", "after": 250, "duration": 800 } }
        ]));

        assert!(errors.is_empty());
        let options = actions[0].options();
        assert_eq!(options.preset, Some(EffectPreset::FlyIn));
        assert_eq!(options.origin, Some(Origin::Left));
        assert_eq!(options.after, Some(250));
        assert_eq!(options.duration, 800);
    }

    #[test]
    fn groups_accept_a_bare_list() {
        let (actions, errors) =
            parse_step_actions(&json!([{ "group": [{ "reveal": ".a" }, { "reveal": ".b" }] }]));

        assert!(errors.is_empty());
        assert_eq!(actions[0].targets(), vec![".a", ".b"]);
    }

    #[test]
    fn groups_accept_an_options_mapping() {
        let (actions, errors) = parse_step_actions(
            &json!([{ "group": { "actions": [{ "reveal": ".a" }], "after": 100 } }]),
        );

        assert!(errors.is_empty());
        assert!(actions[0].is_auto());
    }

    #[test]
    fn an_unknown_action_is_reported_without_dropping_its_neighbours() {
        let (actions, errors) = parse_step_actions(
            &json!([{ "reveal": ".a" }, { "teleport": ".b" }, { "hide": ".a" }]),
        );

        assert_eq!(actions.len(), 2, "the good actions still compile");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("teleport"));
        assert!(errors[0].contains("steps[1]"));
    }

    #[test]
    fn a_missing_target_is_reported() {
        let (actions, errors) = parse_step_actions(&json!([{ "reveal": { "preset": "fade" } }]));
        assert!(actions.is_empty());
        assert!(errors[0].contains("target"));
    }

    #[test]
    fn an_empty_target_is_reported() {
        let (_, errors) = parse_step_actions(&json!([{ "reveal": "   " }]));
        assert!(errors[0].contains("must not be empty"));
    }

    #[test]
    fn a_non_list_steps_value_is_reported() {
        let (actions, errors) = parse_step_actions(&json!({ "reveal": ".a" }));
        assert!(actions.is_empty());
        assert!(errors[0].contains("must be a list"));
    }

    #[test]
    fn an_unparseable_preset_falls_back_to_the_action_default() {
        let (actions, errors) = parse_step_actions(&json!([
            { "reveal": { "target": ".a", "preset": "explode" } }
        ]));

        assert!(errors.is_empty(), "an unknown preset is not fatal");
        assert_eq!(actions[0].options().preset, None);
        assert_eq!(actions[0].options().resolve(EffectKind::Entrance).preset, EffectPreset::Fade);
    }
}
