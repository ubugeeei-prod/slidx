//! Turning marks into markup, and takes into steps.
//!
//! The output side of a mark. Two jobs that share the same walk over the
//! source, which is why they live together: rewriting each mark into a span
//! the renderer passes through, and lifting *takes* — adjacent marks sharing a
//! key — out of the prose and into the step pipeline.

use super::{find_marks, FoundMark, Mark, MARK_ATTRIBUTE};

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

    for FoundMark { mark, start, end, .. } in found {
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

    for FoundMark { mark, start, end, .. } in found {
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
