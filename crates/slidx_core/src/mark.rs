//! Inline marks: `[selected words]{#key .accent color=danger}`.
//!
//! # Why this exists
//!
//! The visual editor has to be able to do what a presentation tool does —
//! select three words and colour them, pick a font for a phrase, animate a
//! fragment of a sentence. Every one of those addresses a *range inside a
//! block*, and Markdown has no way to name one.
//!
//! A mark is that name. It is the smallest addressable unit in a slide, and it
//! is what makes the canvas and the file two views of one document rather than
//! an import and an export.
//!
//! # The round-trip law
//!
//! Marks are written by the editor and read by a human, so both directions
//! have to hold:
//!
//! - [`Mark::to_source`] produces a *canonical* form: attributes in a fixed
//!   order, minimal quoting. Two marks that mean the same thing serialise
//!   identically, so the editor never produces a spurious diff.
//! - Parsing that form yields the same [`Mark`]. Asserted as a property over
//!   generated input, not just on examples.
//!
//! Without canonical output, opening a deck in the editor and closing it again
//! would rewrite unrelated lines — which is the failure that makes people stop
//! trusting a bidirectional tool.
//!
//! # Syntax
//!
//! `[text]{attributes}`, where attributes are space-separated and may be:
//!
//! | Form | Meaning |
//! |---|---|
//! | `#key` | stable identifier, used by `steps:` and `placements:` |
//! | `.name` | style class from the theme |
//! | `key=value` | typed property such as `color=danger` |
//! | `key="two words"` | the same, quoted |
//!
//! `[text](url)` is a link and is left alone: a mark is distinguished by `{`,
//! which CommonMark gives no meaning after `]`. A literal bracket is escaped
//! as `\[`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

mod compile;
mod find;

pub use compile::{compile_marks, resolve_target, stage_takes, strip_marks, StagedTakes};
pub use find::{find_marks, FoundMark};

/// Attribute the compiler writes onto a marked span.
pub const MARK_ATTRIBUTE: &str = "data-slidx-mark";

/// One marked range of inline content.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mark {
    /// The marked text, as written.
    pub text: String,
    /// Stable identifier. Present whenever anything refers to this mark.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Theme classes, in the order written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<String>,
    /// Typed properties. Sorted, so serialisation is canonical.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
}

impl Mark {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Self::default() }
    }

    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn with_class(mut self, class: impl Into<String>) -> Self {
        self.classes.push(class.into());
        self
    }

    pub fn with_property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }

    /// True when the mark carries nothing and would render as plain text.
    ///
    /// The editor drops these rather than leaving `[text]{}` behind when a
    /// user removes the last property from a selection.
    pub fn is_bare(&self) -> bool {
        self.key.is_none() && self.classes.is_empty() && self.properties.is_empty()
    }

    /// The selector `steps:` uses to target this mark.
    pub fn selector(&self) -> Option<String> {
        self.key.as_ref().map(|key| format!("[{MARK_ATTRIBUTE}=\"{key}\"]"))
    }

    /// Canonical source form.
    ///
    /// Order is fixed — key, then classes as written, then properties sorted —
    /// so re-saving a deck never reorders an attribute list and never produces
    /// a diff the author did not ask for.
    pub fn to_source(&self) -> String {
        if self.is_bare() {
            return escape_text(&self.text);
        }

        let mut attributes = Vec::new();

        if let Some(key) = &self.key {
            attributes.push(format!("#{key}"));
        }
        for class in &self.classes {
            attributes.push(format!(".{class}"));
        }
        for (name, value) in &self.properties {
            attributes.push(format!("{name}={}", quote(value)));
        }

        format!("[{}]{{{}}}", escape_text(&self.text), attributes.join(" "))
    }
}

/// Wraps a value in quotes only when it needs them.
fn quote(value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value.chars().any(|c| c.is_whitespace() || matches!(c, '"' | '\'' | '}' | '='));

    if !needs_quotes {
        return value.to_string();
    }

    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Escapes the characters that would otherwise start a mark.
fn escape_text(text: &str) -> String {
    text.replace('\\', "\\\\").replace('[', "\\[").replace(']', "\\]")
}
