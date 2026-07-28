//! The authored half of the step pipeline.
//!
//! A [`StepSource`] is exactly what the author wrote — a flat, order-preserving
//! list of intents. It is deliberately free of resolved state so that the same
//! source can be recompiled after an edit without replaying history.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::preset::{Easing, EffectKind, EffectPreset, Origin};

/// Default animation length, in milliseconds.
///
/// Short enough that a fast presenter never waits on the tool, long enough to
/// read as intentional motion from the back of a room.
pub const DEFAULT_DURATION_MS: u32 = 400;

/// Tuning shared by every action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StepOptions {
    /// Milliseconds to wait before playing. `Some` also means the action plays
    /// automatically instead of consuming a click.
    pub after: Option<u32>,
    pub preset: Option<EffectPreset>,
    pub duration: u32,
    pub easing: Easing,
    pub origin: Option<Origin>,
}

impl Default for StepOptions {
    fn default() -> Self {
        Self {
            after: None,
            preset: None,
            duration: DEFAULT_DURATION_MS,
            easing: Easing::default(),
            origin: None,
        }
    }
}

impl StepOptions {
    /// Resolves the effect this action contributes to a frame.
    pub fn resolve(&self, kind: EffectKind) -> Effect {
        Effect {
            kind,
            preset: self.preset.unwrap_or_else(|| EffectPreset::default_for(kind)),
            duration_ms: self.duration,
            delay_ms: self.after.unwrap_or(0),
            easing: self.easing,
            origin: self.origin,
        }
    }
}

/// A resolved animation attached to one element in one frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    pub kind: EffectKind,
    pub preset: EffectPreset,
    pub duration_ms: u32,
    pub delay_ms: u32,
    pub easing: Easing,
    pub origin: Option<Origin>,
}

impl Default for Effect {
    fn default() -> Self {
        Self {
            kind: EffectKind::default(),
            preset: EffectPreset::default(),
            duration_ms: DEFAULT_DURATION_MS,
            delay_ms: 0,
            easing: Easing::default(),
            origin: None,
        }
    }
}

/// Whether an element is painted in a given frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    /// Present in the layout but not painted, so revealing never reflows.
    Hidden,
    Visible,
}

/// One authored intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StepAction {
    Reveal {
        target: String,
        options: StepOptions,
    },
    Hide {
        target: String,
        options: StepOptions,
    },
    Emphasize {
        target: String,
        options: StepOptions,
    },
    /// Several intents that land on the same click.
    Group {
        actions: Vec<StepAction>,
        options: StepOptions,
    },
}

impl StepAction {
    pub fn reveal(target: impl Into<String>) -> Self {
        Self::Reveal { target: target.into(), options: StepOptions::default() }
    }

    pub fn hide(target: impl Into<String>) -> Self {
        Self::Hide { target: target.into(), options: StepOptions::default() }
    }

    pub fn emphasize(target: impl Into<String>, preset: EffectPreset) -> Self {
        Self::Emphasize {
            target: target.into(),
            options: StepOptions { preset: Some(preset), ..StepOptions::default() },
        }
    }

    pub fn group(actions: Vec<StepAction>) -> Self {
        Self::Group { actions, options: StepOptions::default() }
    }

    /// Plays this action automatically `ms` after the frame it belongs to,
    /// instead of waiting for the presenter to advance.
    pub fn after_ms(mut self, ms: u32) -> Self {
        self.options_mut().after = Some(ms);
        self
    }

    /// Overrides the animation used by this action.
    pub fn with_preset(mut self, preset: EffectPreset) -> Self {
        self.options_mut().preset = Some(preset);
        self
    }

    /// Overrides the animation length, in milliseconds.
    pub fn with_duration(mut self, ms: u32) -> Self {
        self.options_mut().duration = ms;
        self
    }

    /// Sets the direction the effect travels from or towards.
    pub fn with_origin(mut self, origin: Origin) -> Self {
        self.options_mut().origin = Some(origin);
        self
    }

    pub fn options(&self) -> &StepOptions {
        match self {
            Self::Reveal { options, .. }
            | Self::Hide { options, .. }
            | Self::Emphasize { options, .. }
            | Self::Group { options, .. } => options,
        }
    }

    fn options_mut(&mut self) -> &mut StepOptions {
        match self {
            Self::Reveal { options, .. }
            | Self::Hide { options, .. }
            | Self::Emphasize { options, .. }
            | Self::Group { options, .. } => options,
        }
    }

    /// True when the action plays on a timer rather than on a click.
    pub fn is_auto(&self) -> bool {
        self.options().after.is_some()
    }

    /// Every selector this action touches, including nested group members.
    pub fn targets(&self) -> Vec<&str> {
        match self {
            Self::Reveal { target, .. }
            | Self::Hide { target, .. }
            | Self::Emphasize { target, .. } => vec![target.as_str()],
            Self::Group { actions, .. } => actions.iter().flat_map(Self::targets).collect(),
        }
    }
}

/// Automatic staging derived from slide structure rather than explicit actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AutoSteps {
    /// Reveal top-level list items one at a time.
    List,
    /// Reveal every top-level block one at a time.
    Block,
    /// Reveal table rows one at a time.
    Row,
}

impl AutoSteps {
    pub fn as_token(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Block => "block",
            Self::Row => "row",
        }
    }
}

/// Everything the author declared about how a slide advances.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepSource {
    pub actions: Vec<StepAction>,
    pub auto: Option<AutoSteps>,
}

impl StepSource {
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

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

    #[test]
    fn builders_compose() {
        let action = StepAction::reveal(".a")
            .with_preset(EffectPreset::Zoom)
            .with_duration(900)
            .with_origin(Origin::Bottom)
            .after_ms(120);

        let options = action.options();
        assert_eq!(options.preset, Some(EffectPreset::Zoom));
        assert_eq!(options.duration, 900);
        assert_eq!(options.origin, Some(Origin::Bottom));
        assert_eq!(options.after, Some(120));
        assert!(action.is_auto());
    }
}
