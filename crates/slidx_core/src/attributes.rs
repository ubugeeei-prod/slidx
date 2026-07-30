//! The attribute group: `{#key .class prop=value}`.
//!
//! One grammar, written down once. It appears after a span of text as an inline
//! [`Mark`](crate::Mark), after a fence's language to publish a snippet, and on
//! a line of its own to configure the [block](crate::block) below it. Those are
//! three places an author writes the same thing, and a second parser for it
//! would be a second set of answers about what `prop="two words"` means.
//!
//! Both directions live here, because only together do they hold the round-trip
//! law: [`render`] produces a canonical form — key, then classes as written,
//! then properties sorted — and [`parse`] inverts it. Without canonical output,
//! opening a deck in the editor and closing it again reorders attribute lists
//! nobody touched, which is the diff that makes people stop trusting a
//! bidirectional tool.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// What an attribute group carries.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Attributes {
    /// Stable identifier. Present whenever anything refers to this thing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Theme classes, in the order written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<String>,
    /// Typed properties. Sorted, so serialisation is canonical.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
}

impl Attributes {
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

    /// True when the group carries nothing and should not be written at all.
    pub fn is_empty(&self) -> bool {
        self.key.is_none() && self.classes.is_empty() && self.properties.is_empty()
    }

    /// The canonical inside of a group, without the braces.
    pub fn to_source(&self) -> String {
        render(self.key.as_deref(), &self.classes, &self.properties)
    }
}

/// The canonical inside of a group, without the braces.
///
/// Order is fixed so that re-saving a deck never reorders an attribute list and
/// never produces a diff the author did not ask for.
pub fn render(
    key: Option<&str>,
    classes: &[String],
    properties: &BTreeMap<String, String>,
) -> String {
    let mut tokens = Vec::with_capacity(1 + classes.len() + properties.len());

    if let Some(key) = key {
        tokens.push(format!("#{key}"));
    }
    for class in classes {
        tokens.push(format!(".{class}"));
    }
    for (name, value) in properties {
        tokens.push(format!("{name}={}", quote(value)));
    }

    tokens.join(" ")
}

/// Reads the inside of a group.
///
/// `None` when nothing in it is an attribute, which is how a caller tells
/// `{}` — or a paragraph that merely begins with a brace — apart from a group
/// that means something. Forgiving otherwise: a half-typed group exists
/// constantly while someone is editing, and none of them may make content
/// vanish.
pub fn parse(inside: &str) -> Option<Attributes> {
    let mut attributes = Attributes::default();
    let mut saw_one = false;

    for token in tokenize(inside) {
        saw_one = true;

        if let Some(key) = token.strip_prefix('#') {
            if key.is_empty() {
                return None;
            }
            attributes.key = Some(key.to_string());
        } else if let Some(class) = token.strip_prefix('.') {
            if class.is_empty() {
                return None;
            }
            attributes.classes.push(class.to_string());
        } else if let Some((name, value)) = token.split_once('=') {
            if name.is_empty() {
                return None;
            }
            attributes.properties.insert(name.to_string(), unquote(value));
        } else {
            // A bare word is shorthand for a class, so `{accent}` works.
            attributes.classes.push(token);
        }
    }

    saw_one.then_some(attributes)
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

/// Splits an attribute list, keeping quoted values whole.
fn tokenize(attributes: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut escaped = false;

    for character in attributes.chars() {
        match character {
            _ if escaped => {
                current.push(character);
                escaped = false;
            }
            '\\' => escaped = true,
            '"' => {
                quoted = !quoted;
                current.push(character);
            }
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    match trimmed.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) {
        Some(inner) => inner.replace("\\\"", "\"").replace("\\\\", "\\"),
        None => trimmed.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_group_reads_a_key_classes_and_properties() {
        let attributes = parse("#hero .accent .wide color=danger").unwrap();

        assert_eq!(attributes.key.as_deref(), Some("hero"));
        assert_eq!(attributes.classes, vec!["accent", "wide"]);
        assert_eq!(attributes.properties.get("color").map(String::as_str), Some("danger"));
    }

    #[test]
    fn a_bare_word_is_shorthand_for_a_class() {
        assert_eq!(parse("accent").unwrap().classes, vec!["accent"]);
    }

    #[test]
    fn a_quoted_value_stays_one_token() {
        let attributes = parse("title=\"The retry policy\"").unwrap();
        assert_eq!(
            attributes.properties.get("title").map(String::as_str),
            Some("The retry policy")
        );
    }

    #[test]
    fn a_group_carrying_nothing_is_not_a_group() {
        // `{}` is what a half-finished edit looks like, and reading it as an
        // empty attribute set would let the editor write it back.
        assert!(parse("").is_none());
        assert!(parse("   ").is_none());
    }

    #[test]
    fn a_marker_with_no_name_after_it_is_refused() {
        assert!(parse("#").is_none());
        assert!(parse(".").is_none());
        assert!(parse("=value").is_none());
    }

    #[test]
    fn rendering_puts_the_key_first_the_classes_as_written_and_the_properties_sorted() {
        // Canonical output is what keeps re-saving a deck from reordering a list
        // the author never touched.
        let attributes = Attributes::default()
            .with_key("hero")
            .with_class("wide")
            .with_class("accent")
            .with_property("size", "large")
            .with_property("color", "danger");

        assert_eq!(attributes.to_source(), "#hero .wide .accent color=danger size=large");
    }

    #[test]
    fn a_value_is_quoted_only_when_it_needs_to_be() {
        assert_eq!(Attributes::default().with_property("a", "one").to_source(), "a=one");
        assert_eq!(
            Attributes::default().with_property("a", "one two").to_source(),
            "a=\"one two\""
        );
        assert_eq!(Attributes::default().with_property("a", "").to_source(), "a=\"\"");
    }

    #[test]
    fn parsing_a_rendered_group_gives_the_same_attributes_back() {
        let attributes = Attributes::default()
            .with_key("k")
            .with_class("c")
            .with_property("quoted", "two words")
            .with_property("plain", "one");

        assert_eq!(parse(&attributes.to_source()), Some(attributes));
    }

    #[test]
    fn an_empty_group_says_so() {
        assert!(Attributes::default().is_empty());
        assert!(!Attributes::default().with_class("a").is_empty());
    }
}
