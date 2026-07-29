//! Adding and removing entries in a slide's `steps:` list.
//!
//! Two paths, and the reason for both is the diff.
//!
//! When the source already holds a block list of exactly the steps the model
//! reports, an added step is one inserted line and a removed step is one
//! deleted line. Nothing else in the block is read, so the author's own
//! spelling of every other step survives.
//!
//! Otherwise — a flow list, a slide staged with `<!-- step -->` markers, no
//! `steps:` at all — the list is written out from the compiled model. That is
//! a bigger diff, confined to one key, and it is what makes adding a step to a
//! marker-staged slide keep the reveals the author already had: `steps:` takes
//! precedence over markers, so writing it has to carry them across rather than
//! silently replace them.

use slidx_core::{parse_deck, ByteSpan, DeckParseOptions, StepAction};

use crate::edit::EditBuilder;
use crate::frontmatter::{entry, lines, write_key};
use crate::op::{EditError, SlideRef};
use crate::source::DeckSource;

pub(crate) fn add(
    deck: &DeckSource<'_>,
    options: &DeckParseOptions,
    slide: &SlideRef,
    action: &StepAction,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;
    let declared = declared(deck, options, index);

    match block_list(deck, index) {
        Some(list) if list.items.len() == declared.len() => {
            builder.insert(
                list.end,
                format!("{}{}- {}", deck.newline(), list.indent, action.to_source()),
            );
        }
        _ => {
            let mut actions = declared;
            actions.push(action.clone());
            write_list(deck, index, &actions, builder);
        }
    }

    Ok(())
}

pub(crate) fn remove(
    deck: &DeckSource<'_>,
    options: &DeckParseOptions,
    slide: &SlideRef,
    at: usize,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;
    let declared = declared(deck, options, index);

    if at >= declared.len() {
        return Err(EditError::NoSuchStep { index: at, present: declared.len() });
    }

    match block_list(deck, index) {
        // Deleting one item of many is one line. Deleting the last one is not,
        // because `steps:` with nothing under it is no longer a list.
        Some(list) if list.items.len() == declared.len() && declared.len() > 1 => {
            let from = if at == 0 { list.value } else { list.items[at - 1].end };
            builder.delete(ByteSpan::new(from, list.items[at].end));
        }
        _ => {
            let mut actions = declared;
            actions.remove(at);
            write_list(deck, index, &actions, builder);
        }
    }

    Ok(())
}

/// The steps the slide runs, however they are spelled.
///
/// Reads the compiled model rather than the block, so a slide staged with
/// markers reports the reveals they produce.
fn declared(deck: &DeckSource<'_>, options: &DeckParseOptions, index: usize) -> Vec<StepAction> {
    parse_deck(deck.source, options)
        .slides
        .get(index)
        .map(|slide| slide.steps.actions.clone())
        .unwrap_or_default()
}

fn write_list(
    deck: &DeckSource<'_>,
    index: usize,
    actions: &[StepAction],
    builder: &mut EditBuilder<'_>,
) {
    // An empty list rather than a missing key: removing the last step must not
    // change whether the slide declares its staging, only what it declares.
    let value = match actions.is_empty() {
        true => " []".to_string(),
        false => actions
            .iter()
            .map(|action| format!("{}  - {}", deck.newline(), action.to_source()))
            .collect(),
    };

    write_key(deck, index, "steps", &value, builder);
}

/// A `steps:` value written as a YAML block list.
#[derive(Debug)]
struct BlockList {
    /// Indentation the items are written at, so a new one matches.
    indent: String,
    /// Where the value begins, just after the colon.
    value: usize,
    /// Where the last item ends, which is where a new one goes.
    end: usize,
    items: Vec<ByteSpan>,
}

fn block_list(deck: &DeckSource<'_>, index: usize) -> Option<BlockList> {
    let block = deck.at(index).frontmatter?;
    let text = block.slice(deck.source);
    let found = entry(text, "steps")?;
    let base = block.start + found.value.start;

    let mut items: Vec<ByteSpan> = Vec::new();
    let mut indent: Option<String> = None;

    for (start, line) in lines(found.value.slice(text)) {
        let body = line.trim_start();
        let written_at = &line[..line.len() - body.len()];
        let opens_item =
            body.starts_with("- ") && indent.as_deref().is_none_or(|at| at == written_at);

        if opens_item {
            indent.get_or_insert_with(|| written_at.to_string());
            items.push(ByteSpan::new(base + start, base + start + line.trim_end().len()));
        } else if !line.trim().is_empty() {
            // A continuation line — a nested mapping under one item — belongs
            // to the item above it.
            if let Some(last) = items.last_mut() {
                last.end = base + start + line.trim_end().len();
            }
        }
    }

    Some(BlockList { indent: indent?, value: base, end: items.last()?.end, items })
}
