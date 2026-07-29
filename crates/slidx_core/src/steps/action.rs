//! The authored half of the step pipeline.
//!
//! A [`StepSource`] is exactly what the author wrote — a flat, order-preserving
//! list of intents. It is deliberately free of resolved state so that the same
//! source can be recompiled after an edit without replaying history.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::preset::{EffectPreset, Origin};
use super::timing::StepOptions;

/// Whether an element is painted in a given frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    /// Present in the layout but not painted, so revealing never reflows.
    Hidden,
    Visible,
}

/// A change to an element that is already on screen.
///
/// Reveal and hide cover "not there yet" and "gone". This covers the third
/// thing a presenter does, which is to change something the audience is
/// already looking at — a number that updates, a label that turns red, a line
/// of code that becomes the focus.
///
/// It is the counterpart of a mark: a mark names a range, a patch says what
/// that range becomes. Absent fields mean "leave alone", so a patch that only
/// changes colour does not have to restate the text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Patch {
    /// Replaces the element's text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Data properties to set. Sorted, so a patch serialises canonically and
    /// the editor never produces a diff nobody asked for.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
}

impl Patch {
    pub fn content(text: impl Into<String>) -> Self {
        Self { content: Some(text.into()), ..Self::default() }
    }

    pub fn with_property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_none() && self.properties.is_empty()
    }
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
    /// Changes an element that is already visible, in place.
    Set {
        target: String,
        patch: Patch,
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

    pub fn set(target: impl Into<String>, patch: Patch) -> Self {
        Self::Set { target: target.into(), patch, options: StepOptions::default() }
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
            | Self::Set { options, .. }
            | Self::Group { options, .. } => options,
        }
    }

    fn options_mut(&mut self) -> &mut StepOptions {
        match self {
            Self::Reveal { options, .. }
            | Self::Hide { options, .. }
            | Self::Emphasize { options, .. }
            | Self::Set { options, .. }
            | Self::Group { options, .. } => options,
        }
    }

    /// True when the action plays on a timer rather than on a click.
    pub fn is_auto(&self) -> bool {
        self.options().after.is_some()
    }

    /// Canonical `steps:` source for this action, without the leading `- `.
    ///
    /// The inverse of [`parse_step_actions`](super::parse::parse_step_actions),
    /// and the reason the editor's timeline can write frontmatter at all. Two
    /// forms rather than one: an action with default timing is a single
    /// `reveal: ".a"`, because that is what an author writes by hand and a
    /// deck that gains five keys of defaults on being opened in the editor is
    /// a deck nobody will open in the editor twice.
    ///
    /// Anything with options is written as a flow mapping so an action always
    /// occupies exactly one line, which keeps a reordered timeline to a diff
    /// of moved lines.
    pub fn to_source(&self) -> String {
        let timed = *self.options() != StepOptions::default();

        let (name, mut fields) = match self {
            Self::Reveal { target, .. } => ("reveal", vec![field("target", target)]),
            Self::Hide { target, .. } => ("hide", vec![field("target", target)]),
            Self::Emphasize { target, .. } => ("emphasize", vec![field("target", target)]),
            Self::Set { target, patch, .. } => ("set", set_fields(target, patch)),
            Self::Group { actions, .. } => {
                let members: Vec<String> =
                    actions.iter().map(|action| format!("{{ {} }}", action.to_source())).collect();
                let list = format!("[{}]", members.join(", "));

                // A group with default timing is a bare list, which is both the
                // shorter spelling and the one the parser documents first.
                if !timed {
                    return format!("group: {list}");
                }
                ("group", vec![("actions".to_string(), list)])
            }
        };

        // A `set` always needs its mapping — a bare target says nothing about
        // what changed — and so does anything that carries timing.
        if !timed
            && matches!(self, Self::Reveal { .. } | Self::Hide { .. } | Self::Emphasize { .. })
        {
            return format!("{name}: {}", fields.remove(0).1);
        }

        fields.extend(self.options().to_fields());
        let written: Vec<String> =
            fields.iter().map(|(key, value)| format!("{key}: {value}")).collect();

        format!("{name}: {{ {} }}", written.join(", "))
    }

    /// Every selector this action touches, including nested group members.
    pub fn targets(&self) -> Vec<&str> {
        match self {
            Self::Reveal { target, .. }
            | Self::Hide { target, .. }
            | Self::Emphasize { target, .. }
            | Self::Set { target, .. } => vec![target.as_str()],
            Self::Group { actions, .. } => actions.iter().flat_map(Self::targets).collect(),
        }
    }
}

fn field(key: &str, value: &str) -> (String, String) {
    (key.to_string(), yaml_string(value))
}

fn set_fields(target: &str, patch: &Patch) -> Vec<(String, String)> {
    let mut fields = vec![field("target", target)];

    if let Some(content) = &patch.content {
        fields.push(field("text", content));
    }
    for (name, value) in &patch.properties {
        fields.push(field(name, value));
    }

    fields
}

/// Quotes a value for a YAML flow mapping.
///
/// Always quoted, never conditionally: selectors are made of the characters a
/// flow mapping reserves — `[`, `{`, `,`, `"`, `:` — and a rule that decides
/// per value is a rule that eventually decides wrong on the one selector
/// nobody tested.
fn yaml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steps::parse::parse_step_actions;

    /// Reads back a `steps:` list of one written action.
    fn reparse(action: &StepAction) -> Vec<StepAction> {
        let yaml = format!("- {}\n", action.to_source());
        let value: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("valid YAML");
        let (actions, errors) = parse_step_actions(&serde_json::to_value(value).unwrap());

        assert!(errors.is_empty(), "{yaml} did not read back: {errors:?}");
        actions
    }

    #[test]
    fn an_action_without_options_writes_the_short_form() {
        assert_eq!(StepAction::reveal(".a").to_source(), r#"reveal: ".a""#);
        assert_eq!(StepAction::hide("[data-x=\"1\"]").to_source(), r#"hide: "[data-x=\"1\"]""#);
    }

    #[test]
    fn every_action_reads_back_as_the_action_that_wrote_it() {
        // The editor's timeline writes `steps:` and the parser reads it. If
        // those two ever disagree, a deck changes meaning by being saved.
        let cases = vec![
            StepAction::reveal(".a"),
            // Selectors are written resolved. `#hero` is authored shorthand
            // that the parser expands, so it is not a fixed point and never
            // reaches a written action.
            StepAction::hide(crate::mark::resolve_target("#hero")),
            StepAction::emphasize(".b", EffectPreset::Pulse),
            StepAction::reveal(".c")
                .with_preset(EffectPreset::FlyIn)
                .with_origin(Origin::Left)
                .with_duration(800)
                .after_ms(250),
            StepAction::set(".d", Patch::content("42").with_property("color", "danger, bold")),
            StepAction::group(vec![StepAction::reveal(".e"), StepAction::hide(".f")]),
            StepAction::group(vec![StepAction::reveal(".g")]).after_ms(100),
        ];

        for action in cases {
            assert_eq!(reparse(&action), vec![action.clone()], "{}", action.to_source());
        }
    }

    #[test]
    fn a_target_that_looks_like_yaml_survives_being_written() {
        // Selectors are full of brackets, braces, commas, and quotes — every
        // character that means something else in a flow mapping.
        let action = StepAction::reveal("[data-slidx-mark=\"a, b\"] > li").with_duration(700);
        assert_eq!(reparse(&action), vec![action]);
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
