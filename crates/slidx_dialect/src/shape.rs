//! Frontmatter values whose shape the reader cannot use.
//!
//! Every reader in `slidx_core::frontmatter` is an `Option`: it returns the
//! value when the shape is right and `None` when it is not, because a parse must
//! never fail. So `duration: fast` produces a deck with no countdown, and
//! nothing anywhere says why — the reader that would have complained is the one
//! that returned `None`.
//!
//! This module asks the only question that distinguishes a wrong value from an
//! absent one: **was the key written at all?** A key present whose reader
//! declined is a value that was dropped.

use slidx_core::{frontmatter, AspectRatio, Deck, Diagnostic, Diagnostics, SourceSpan};

/// Frontmatter keys that hold a duration, and what each one is for.
///
/// Two, and they are read by different things — the presenter's countdown and
/// the per-slide budget — so the message names the one that was lost rather than
/// saying "a duration" and leaving the author to work out which.
const DURATIONS: [(&str, &str); 2] =
    [("duration", "the talk's slot length"), ("budget", "this slide's share of the slot")];

pub fn check(deck: &Deck, sink: &mut Diagnostics) {
    check_aspect(deck, sink);

    for slide in &deck.slides {
        for (key, purpose) in DURATIONS {
            check_duration(&slide.frontmatter, key, purpose, slide.index, slide.source_line, sink);
        }
    }
}

/// Flags a duration the reader dropped.
fn check_duration(
    matter: &serde_json::Value,
    key: &str,
    purpose: &str,
    index: u32,
    line: u32,
    sink: &mut Diagnostics,
) {
    let Some(field) = frontmatter::field(matter, key) else {
        return;
    };
    if frontmatter::duration_seconds(matter, key).is_some() {
        return;
    }

    sink.push(
        Diagnostic::warning(
            "dialect/invalid-duration",
            format!("`{key}` is {purpose}, and `{}` is not a length of time", written(field)),
        )
        .at(SourceSpan::line(line).on_slide(index))
        .with_help("write minutes as `20m`, seconds as `90s`, or a clock as `20:00`"),
    );
}

/// Flags an aspect ratio no reader will ever see.
///
/// Only the shape: a *string* that is not a ratio is already
/// `deck/unknown-aspect`, and saying it twice would have the author fix it
/// twice. What is left is a value that is not a string at all — `aspect: 16`,
/// which YAML reads as a number and every reader of the key then skips.
fn check_aspect(deck: &Deck, sink: &mut Diagnostics) {
    let matter = &deck.meta.raw;

    let Some(field) =
        frontmatter::field(matter, "aspect").or_else(|| frontmatter::field(matter, "aspectRatio"))
    else {
        return;
    };
    if field.as_str().is_some() {
        return;
    }

    sink.push(
        Diagnostic::warning(
            "dialect/invalid-aspect",
            format!("`aspect` is a ratio, and `{}` is not one", written(field)),
        )
        .at(SourceSpan::default().on_slide(0))
        .with_help(format!(
            "quote it, so YAML keeps the colon: `aspect: \"{}\"`",
            AspectRatio::default().as_token()
        )),
    );
}

/// A frontmatter value, as it reads back to the person who wrote it.
///
/// JSON rather than YAML because the deck's frontmatter is kept as JSON, and a
/// string is unquoted because the author did not write the quotes — a message
/// that says `` `"fast"` `` when the file says `fast` sends somebody hunting for
/// quotes that are not there.
fn written(field: &serde_json::Value) -> String {
    field.as_str().map(str::to_string).unwrap_or_else(|| field.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn found(source: &str) -> Vec<String> {
        let deck = parse_deck(source, &DeckParseOptions::default());
        let mut sink = Diagnostics::default();
        check(&deck, &mut sink);

        sink.iter().map(|diagnostic| diagnostic.code.clone()).collect()
    }

    fn one(source: &str) -> Diagnostic {
        let deck = parse_deck(source, &DeckParseOptions::default());
        let mut sink = Diagnostics::default();
        check(&deck, &mut sink);

        assert_eq!(sink.len(), 1, "{:?}", sink);
        sink.as_slice()[0].clone()
    }

    #[test]
    fn a_duration_nobody_can_read_is_reported_rather_than_dropped() {
        // Today this deck has no countdown and says nothing about why. The
        // speaker finds out when the timer never appears.
        let diagnostic = one("---\nduration: fast\n---\n\n# One\n");

        assert_eq!(diagnostic.code, "dialect/invalid-duration");
        assert!(diagnostic.message.contains("fast"), "{}", diagnostic.message);
        assert!(diagnostic.message.contains("slot length"), "it says which key was lost");
    }

    #[test]
    fn a_budget_nobody_can_read_is_reported_too() {
        let diagnostic = one("# One\n\n---\nbudget: soon\n---\n\n# Two\n");

        assert_eq!(diagnostic.code, "dialect/invalid-duration");
        assert_eq!(diagnostic.span.slide_index, Some(1), "on the slide that carries it");
    }

    #[test]
    fn every_notation_a_cfp_uses_is_accepted_in_silence() {
        for value in ["1500", "25m", "25:00", "1h30m", "90s"] {
            assert!(
                found(&format!("---\nduration: {value}\n---\n\n# One\n")).is_empty(),
                "{value} was rejected"
            );
        }
    }

    #[test]
    fn a_key_nobody_wrote_is_not_a_mistake() {
        // The whole distinction this module is built on: an absent key and a
        // wrong one look identical to the reader, and only one is a problem.
        assert!(found("# One\n").is_empty());
    }

    #[test]
    fn a_duration_written_in_kebab_case_is_still_read() {
        // The parser accepts either spelling, so a check that knew only one
        // would report a working deck.
        assert!(found("---\nbudget: 90s\n---\n\n# One\n").is_empty());
    }

    #[test]
    fn an_aspect_that_is_not_a_string_is_reported() {
        // `aspect: 16` is a number, so every reader of the key skips it and the
        // deck quietly renders at the default.
        let diagnostic = one("---\naspect: 16\n---\n\n# One\n");

        assert_eq!(diagnostic.code, "dialect/invalid-aspect");
        assert!(diagnostic.help.as_ref().unwrap().contains("quote"), "it says what to type");
    }

    #[test]
    fn an_aspect_the_parser_already_complains_about_is_not_complained_about_twice() {
        // `16x9` is a string that is not a ratio, which is `deck/unknown-aspect`.
        // A second finding would have the author fix one mistake twice.
        let deck = parse_deck("---\naspect: 16x9\n---\n\n# One\n", &DeckParseOptions::default());

        assert!(deck.diagnostics.iter().any(|d| d.code == "deck/unknown-aspect"));
        assert!(found("---\naspect: 16x9\n---\n\n# One\n").is_empty());
    }

    #[test]
    fn an_unquoted_ratio_is_read_by_yaml_as_a_string_and_is_fine() {
        // `aspect: 16:9` without quotes is a plain scalar, so it works. Flagging
        // it would be a check reporting a deck that renders correctly.
        assert!(found("---\naspect: 16:9\n---\n\n# One\n").is_empty());
        assert!(found("---\naspect: \"4:3\"\n---\n\n# One\n").is_empty());
    }

    #[test]
    fn the_aspect_check_reads_both_spellings_of_the_key() {
        assert_eq!(found("---\naspectRatio: 16\n---\n\n# One\n").len(), 1);
    }

    #[test]
    fn a_value_is_quoted_back_the_way_the_author_wrote_it() {
        // A message that says `"fast"` when the file says `fast` sends somebody
        // looking for quotes that are not there.
        assert!(one("---\nduration: fast\n---\n\n# One\n").message.contains("`fast`"));
        assert!(one("---\naspect: 16\n---\n\n# One\n").message.contains("`16`"));
    }
}
