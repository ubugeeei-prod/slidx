//! Writing a deck, reachable from JavaScript.
//!
//! The editor's UI is TypeScript and every byte it writes is computed here, in
//! [`slidx_edit`]. That is the whole boundary: an operation goes across as
//! JSON, a spliced source comes back, and no code on the other side ever
//! decides what Markdown should look like. The moment there are two writers the
//! round-trip guarantee is gone and it does not come back.
//!
//! # Why an error is a value
//!
//! The editor sends operations built from a deck it parsed a keystroke ago, so
//! naming a slide that has since been deleted is ordinary traffic rather than a
//! bug. Throwing would make every call site handle it as an exception; instead
//! the result carries the source unchanged and says what was missing.
//!
//! Only a malformed options or operation object throws, because that is a
//! caller that cannot be talked to at all.
//!
//! # Why the result carries slide spans
//!
//! A deck is usually stored one slide per file and edited as one joined source,
//! so whoever writes the result back to disk has to cut it up again. Cutting it
//! anywhere but the seams the operations already agreed on would be a second
//! opinion about the file's structure, so the spans come back from the same
//! call that produced the source they measure.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use slidx_core::ByteSpan;
use slidx_edit::{Edit, EditError, EditOp};

use crate::parse_options;

/// Parse settings an edit needs. The separator is the only one that changes
/// which bytes a slide is.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EditOptions {
    pub separator: Option<String>,
}

/// A deck source, after an operation was asked for.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditResult {
    /// The spliced source. The original, byte for byte, when nothing changed.
    pub source: String,
    /// The edit that takes this one back, for the editor's undo stack. Empty
    /// when the operation asked for what the source already said.
    pub undo: Edit,
    /// Where each slide's bytes are in `source`.
    pub slides: Vec<ByteSpan>,
    /// What the operation named that the source does not have.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<EditError>,
}

/// Works out which bytes an operation would change, without changing them.
#[wasm_bindgen(js_name = planEdit)]
pub fn plan_edit(source: &str, op: JsValue, options: JsValue) -> Result<JsValue, JsError> {
    let (op, options) = read(op, options)?;
    let planned = slidx_edit::plan(source, &parse_options(options.separator.as_deref()), &op);

    to_js(&planned.unwrap_or_default())
}

/// The source with one operation applied, and the edit that takes it back.
#[wasm_bindgen(js_name = applyEdit)]
pub fn apply_edit(source: &str, op: JsValue, options: JsValue) -> Result<JsValue, JsError> {
    let (op, options) = read(op, options)?;

    to_js(&applied(source, &op, &options))
}

/// Applies an edit taken off an undo stack, and hands back the one that does it
/// again.
///
/// Redo is undo of undo, so one function serves both directions and neither
/// needs the operation that started it.
#[wasm_bindgen(js_name = revertEdit)]
pub fn revert_edit(source: &str, edit: JsValue, options: JsValue) -> Result<JsValue, JsError> {
    let edit: Edit = serde_wasm_bindgen::from_value(edit)
        .map_err(|error| JsError::new(&format!("invalid edit: {error}")))?;
    let options: EditOptions = read_options(options)?;

    to_js(&reverted(source, &edit, &options))
}

/// Where each slide's bytes are, for a caller that has to write the source back
/// to the files it came from.
#[wasm_bindgen(js_name = slideSpans)]
pub fn slide_spans(source: &str, options: JsValue) -> Result<JsValue, JsError> {
    let options: EditOptions = read_options(options)?;

    to_js(&spans(source, &options))
}

fn applied(source: &str, op: &EditOp, options: &EditOptions) -> EditResult {
    match slidx_edit::plan(source, &parse_options(options.separator.as_deref()), op) {
        Ok(edit) => {
            let next = edit.apply(source);
            let undo = edit.invert(source);

            EditResult { slides: spans(&next, options), source: next, undo, error: None }
        }
        Err(error) => EditResult {
            slides: spans(source, options),
            source: source.to_string(),
            undo: Edit::default(),
            error: Some(error),
        },
    }
}

fn reverted(source: &str, edit: &Edit, options: &EditOptions) -> EditResult {
    let next = edit.apply(source);
    let undo = edit.invert(source);

    EditResult { slides: spans(&next, options), source: next, undo, error: None }
}

fn spans(source: &str, options: &EditOptions) -> Vec<ByteSpan> {
    slidx_edit::slide_spans(source, &parse_options(options.separator.as_deref()))
}

fn read(op: JsValue, options: JsValue) -> Result<(EditOp, EditOptions), JsError> {
    let op: EditOp = serde_wasm_bindgen::from_value(op)
        .map_err(|error| JsError::new(&format!("invalid operation: {error}")))?;

    Ok((op, read_options(options)?))
}

fn read_options(options: JsValue) -> Result<EditOptions, JsError> {
    if options.is_undefined() || options.is_null() {
        return Ok(EditOptions::default());
    }

    serde_wasm_bindgen::from_value(options)
        .map_err(|error| JsError::new(&format!("invalid options: {error}")))
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsError> {
    serde_wasm_bindgen::to_value(value).map_err(|error| JsError::new(&error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_edit::SlideRef;

    const DECK: &str = "---\ntitle: T\n---\n\n#   One\n\nBody.\n\n---\n\n# Two\n";

    fn options() -> EditOptions {
        EditOptions::default()
    }

    #[test]
    fn applying_an_operation_returns_the_spliced_source_and_the_edit_that_takes_it_back() {
        let op = EditOp::SetHeading { slide: 0.into(), text: "Retitled".into() };
        let result = applied(DECK, &op, &options());

        assert_eq!(result.source, "---\ntitle: T\n---\n\n#   Retitled\n\nBody.\n\n---\n\n# Two\n");
        assert!(result.error.is_none());
        assert_eq!(reverted(&result.source, &result.undo, &options()).source, DECK);
    }

    #[test]
    fn an_operation_that_asks_for_what_the_source_already_says_comes_back_with_nothing_to_undo() {
        // The caller writes a file only when there is something to write, so
        // an empty undo is how it knows to leave the file's timestamp alone.
        let op = EditOp::SetHeading { slide: 1.into(), text: "Two".into() };
        let result = applied(DECK, &op, &options());

        assert_eq!(result.source, DECK);
        assert!(result.undo.is_empty());
    }

    #[test]
    fn naming_a_slide_that_is_gone_comes_back_as_a_value_rather_than_a_throw() {
        // The editor sends operations built from a deck it parsed a keystroke
        // ago. That race is ordinary traffic, not an exception.
        let op = EditOp::RemoveSlide { slide: SlideRef::Id("deleted".into()) };
        let result = applied(DECK, &op, &options());

        assert_eq!(result.source, DECK);
        assert!(result.undo.is_empty());
        assert_eq!(result.error, Some(EditError::NoSuchSlide { slide: "deleted".into() }));
    }

    #[test]
    fn the_spans_measure_the_source_that_came_back_with_them() {
        let op = EditOp::InsertSlide { at: 1, body: "# Inserted".into() };
        let result = applied(DECK, &op, &options());

        let text: Vec<&str> = result.slides.iter().map(|span| span.slice(&result.source)).collect();
        assert_eq!(text, ["#   One\n\nBody.", "# Inserted", "# Two"]);
    }

    #[test]
    fn reverting_an_undo_hands_back_the_edit_that_does_it_again() {
        let op = EditOp::SetNotes { slide: 0.into(), notes: "said out loud".into() };
        let done = applied(DECK, &op, &options());

        let undone = reverted(&done.source, &done.undo, &options());
        assert_eq!(undone.source, DECK);

        let redone = reverted(&undone.source, &undone.undo, &options());
        assert_eq!(redone.source, done.source);
    }

    #[test]
    fn a_deck_written_with_a_different_separator_is_still_cut_at_its_own_slides() {
        let options = EditOptions { separator: Some("===".into()) };
        let source = "# One\n\n---\n\n# Still One\n\n===\n\n# Two\n";

        assert_eq!(spans(source, &options).len(), 2);
    }
}
