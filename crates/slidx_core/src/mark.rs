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

/// A mark found in a source string, with the byte range it occupies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundMark {
    pub mark: Mark,
    pub start: usize,
    pub end: usize,
}

/// Finds every mark in a string, in source order.
///
/// Links are skipped, escaped brackets are skipped, and an unterminated mark
/// is left as literal text — a half-typed mark in the editor must not make the
/// rest of the slide disappear.
pub fn find_marks(source: &str) -> Vec<FoundMark> {
    let bytes = source.as_bytes();
    let mut marks = Vec::new();
    let mut index = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'\\' => {
                index += 2;
                continue;
            }
            b'[' => {}
            _ => {
                index += 1;
                continue;
            }
        }

        let Some(text_end) = matching_bracket(bytes, index) else {
            index += 1;
            continue;
        };

        // `]{` is the whole distinction from a link. CommonMark gives `{` no
        // meaning here, so claiming it cannot break an existing document.
        if bytes.get(text_end + 1) != Some(&b'{') {
            index = text_end + 1;
            continue;
        }

        let Some(attributes_end) = source[text_end + 1..].find('}').map(|at| text_end + 1 + at)
        else {
            index = text_end + 1;
            continue;
        };

        let Some(mark) = build(&source[index + 1..text_end], &source[text_end + 2..attributes_end])
        else {
            index = text_end + 1;
            continue;
        };

        marks.push(FoundMark { mark, start: index, end: attributes_end + 1 });
        index = attributes_end + 1;
    }

    marks
}

/// Index of the `]` closing the `[` at `open`, respecting nesting and escapes.
fn matching_bracket(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = open;

    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 1,
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }

    None
}

fn build(text: &str, attributes: &str) -> Option<Mark> {
    let mut mark = Mark::new(unescape(text));
    let mut saw_attribute = false;

    for token in tokenize(attributes) {
        saw_attribute = true;
        if let Some(key) = token.strip_prefix('#') {
            if key.is_empty() {
                return None;
            }
            mark.key = Some(key.to_string());
        } else if let Some(class) = token.strip_prefix('.') {
            if class.is_empty() {
                return None;
            }
            mark.classes.push(class.to_string());
        } else if let Some((name, value)) = token.split_once('=') {
            if name.is_empty() {
                return None;
            }
            mark.properties.insert(name.to_string(), unquote(value));
        } else {
            // A bare word is shorthand for a class, so `[x]{accent}` works.
            mark.classes.push(token);
        }
    }

    saw_attribute.then_some(mark)
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

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut escaped = false;

    for character in text.chars() {
        if escaped {
            out.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            out.push(character);
        }
    }

    out
}

/// Rewrites every mark in a slide body into a span the renderer passes through.
///
/// Marks without a key get one from `next_key`, because an unkeyed mark cannot
/// be animated, restyled, or found again by the editor.
pub fn compile_marks(source: &str, next_key: &mut u32) -> String {
    let found = find_marks(source);
    if found.is_empty() {
        return source.to_string();
    }

    let mut out = String::with_capacity(source.len() + found.len() * 48);
    let mut cursor = 0usize;

    for FoundMark { mark, start, end } in found {
        out.push_str(&source[cursor..start]);
        out.push_str(&compile(&mark, next_key));
        cursor = end;
    }

    out.push_str(&source[cursor..]);
    out
}

fn compile(mark: &Mark, next_key: &mut u32) -> String {
    let key = mark.key.clone().unwrap_or_else(|| {
        let key = format!("m{next_key}");
        *next_key += 1;
        key
    });

    let mut html = format!("<span {MARK_ATTRIBUTE}=\"{key}\"");

    if !mark.classes.is_empty() {
        let classes: Vec<String> =
            mark.classes.iter().map(|class| format!("slidx-{class}")).collect();
        html.push_str(&format!(" class=\"{}\"", classes.join(" ")));
    }

    // Properties become data attributes so the theme decides what they mean.
    // Emitting resolved CSS here would bake one theme's answer into the HTML.
    for (name, value) in &mark.properties {
        html.push_str(&format!(" data-slidx-{}=\"{}\"", name, escape_html(value)));
    }

    html.push('>');
    html.push_str(&escape_html(&mark.text));
    html.push_str("</span>");
    html
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Replaces every mark with its plain text.
///
/// Used wherever a slide's words are needed without its styling: titles, the
/// outline, OG images, the published description, and search. Those all want
/// "Making decks fast", never `Making [decks]{.accent} fast`.
pub fn strip_marks(source: &str) -> String {
    let found = find_marks(source);
    if found.is_empty() {
        return source.to_string();
    }

    let mut out = String::with_capacity(source.len());
    let mut cursor = 0usize;

    for FoundMark { mark, start, end } in found {
        out.push_str(&source[cursor..start]);
        out.push_str(&mark.text);
        cursor = end;
    }

    out.push_str(&source[cursor..]);
    out
}

/// Resolves a `steps:` target written as `#key` into a mark selector.
///
/// Authors and the editor both refer to a mark by the key they can see in the
/// source, rather than by the attribute selector it compiles to. Anything that
/// is already a selector is passed through untouched.
pub fn resolve_target(target: &str) -> String {
    match target.strip_prefix('#') {
        Some(key) if !key.is_empty() && !key.contains([' ', '[', ']', '.']) => {
            format!("[{MARK_ATTRIBUTE}=\"{key}\"]")
        }
        _ => target.to_string(),
    }
}

/// A slide body with its takes lifted out into step actions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StagedTakes {
    /// The body with every take after the first removed.
    pub content: String,
    /// One `Set` per removed take, in source order.
    pub actions: Vec<crate::steps::StepAction>,
    /// Keys used more than once without being adjacent, which is almost
    /// always a copy-paste mistake rather than a take.
    pub ambiguous_keys: Vec<String>,
}

/// Lifts *takes* out of a slide body.
///
/// A take is the answer to "how do I change something that is already on
/// screen" in prose rather than in frontmatter. Writing the same key twice,
/// adjacently, means one element with two successive states:
///
/// ```md
/// The answer is [10]{#count}[42]{#count}.
/// ```
///
/// The first take stays in the markup and becomes the element. Every later one
/// is removed and becomes a `Set` step, so the compiled slide holds **one** DOM
/// node whose text changes — not two nodes that swap. That distinction is the
/// whole point: a presenter changing a number wants the number to change, and
/// anything referring to `#count` keeps referring to the same thing.
///
/// Takes must be adjacent, separated by nothing but whitespace. Two marks
/// sharing a key from opposite ends of a slide are reported as ambiguous
/// instead, because that is a duplicated key rather than a sequence.
pub fn stage_takes(source: &str) -> StagedTakes {
    use crate::steps::{EffectPreset, StepAction, StepOptions};

    let found = find_marks(source);
    if found.len() < 2 {
        return StagedTakes { content: source.to_string(), ..StagedTakes::default() };
    }

    let mut staged = StagedTakes::default();
    let mut removed: Vec<(usize, usize)> = Vec::new();

    for (position, current) in found.iter().enumerate() {
        let Some(key) = &current.mark.key else { continue };

        let Some(previous) = position.checked_sub(1).map(|index| &found[index]) else { continue };
        if previous.mark.key.as_ref() != Some(key) {
            // Not following a take of the same key. If this key appears
            // anywhere else at all, the author has reused it by accident.
            if found.iter().filter(|other| other.mark.key.as_ref() == Some(key)).count() > 1
                && !staged.ambiguous_keys.contains(key)
            {
                staged.ambiguous_keys.push(key.clone());
            }
            continue;
        }

        if !source[previous.end..current.start].trim().is_empty() {
            if !staged.ambiguous_keys.contains(key) {
                staged.ambiguous_keys.push(key.clone());
            }
            continue;
        }

        let mut patch = crate::steps::Patch::content(current.mark.text.clone());
        for (name, value) in &current.mark.properties {
            patch = patch.with_property(name, value);
        }
        if !current.mark.classes.is_empty() {
            patch = patch.with_property("class", current.mark.classes.join(" "));
        }

        staged.actions.push(StepAction::Set {
            target: format!("[{MARK_ATTRIBUTE}=\"{key}\"]"),
            patch,
            // A value changing in place reads as a cross-fade. Pulsing it —
            // the usual emphasis default — draws the eye to motion rather than
            // to the new value.
            options: StepOptions { preset: Some(EffectPreset::Fade), ..StepOptions::default() },
        });

        // Whitespace before the take goes with it, so removing a take never
        // leaves a double space behind.
        removed.push((previous.end, current.end));
    }

    if removed.is_empty() {
        staged.content = source.to_string();
        return staged;
    }

    let mut content = String::with_capacity(source.len());
    let mut cursor = 0usize;
    for (start, end) in removed {
        content.push_str(&source[cursor..start]);
        cursor = end;
    }
    content.push_str(&source[cursor..]);
    staged.content = content;

    staged
}
