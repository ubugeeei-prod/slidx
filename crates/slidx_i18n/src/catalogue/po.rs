//! The catalogue as a Gettext PO file.
//!
//! Not a format invented here, and the reasons are all about who has to read it.
//!
//! - **A reviewer.** PO is line-oriented, so a translation change arrives in a
//!   pull request as a diff about the lines that changed. XLIFF is the other
//!   standard and is XML: a one-word change becomes a diff inside a tree, which
//!   is the same failure this repository avoids by splicing rather than
//!   re-serialising.
//! - **A translator.** Every translation tool already opens PO. That is the
//!   whole hook slidx offers — there is no machine translation in here and no
//!   network call, so the catalogue has to be a file somebody else's tool can
//!   already work on.
//! - **A deck.** `msgctxt` carries the segment's address, which is what lets two
//!   slides that both say "Demo" hold two translations. A flat key-value format
//!   cannot express that without inventing a key convention, which is inventing
//!   a format.
//!
//! Only the subset a deck needs is implemented: context, source, translation,
//! extracted comments and a reference. No plural forms — a slide has none — and
//! no `#, fuzzy` handling, because a fuzzy translation is one a person has not
//! approved and applying it would put an unapproved sentence on a projector.
//! Such an entry is read as its translation; the flag round-trips as an ordinary
//! comment rather than being silently dropped.

use super::{Catalogue, Entry};

/// The catalogue as PO text.
pub fn render(catalogue: &Catalogue) -> String {
    let mut out = String::new();

    out.push_str("# Translation catalogue for a slidx deck.\n");
    out.push_str("#\n");
    out.push_str("# %1, %2 … stand for the parts of a slide that must not be translated: mark\n");
    out.push_str(
        "# keys, inline code, URLs and image paths, HTML tags, step markers. Keep every\n",
    );
    out.push_str("# one of them, and move them if the grammar needs you to. `#.` lines say what\n");
    out.push_str("# each stands for. Write a literal percent before a digit as %%.\n");
    out.push_str("#\n");
    out.push_str("# An empty msgstr leaves that string in the deck's original language.\n");
    out.push_str("msgid \"\"\nmsgstr \"\"\n");
    out.push_str("\"Content-Type: text/plain; charset=UTF-8\\n\"\n");
    out.push_str(&format!("\"Language: {}\\n\"\n", catalogue.lang));

    if !catalogue.deck.is_empty() {
        out.push_str(&format!("\"X-Slidx-Deck: {}\\n\"\n", catalogue.deck));
    }

    for entry in &catalogue.entries {
        out.push('\n');

        for note in &entry.notes {
            out.push_str(&format!("#. {note}\n"));
        }
        if !entry.reference.is_empty() {
            out.push_str(&format!("#: {}\n", entry.reference));
        }

        out.push_str(&quoted("msgctxt", &entry.context));
        out.push_str(&quoted("msgid", &entry.source));
        out.push_str(&quoted("msgstr", &entry.target));
    }

    out
}

/// A catalogue read back from PO text.
///
/// Total, like everything else that reads a file an author edits: a malformed
/// line is skipped rather than refused, because a catalogue is hand-edited and a
/// parse error that drops the whole translation the morning of a talk is the
/// worst possible answer.
pub fn parse(text: &str) -> Catalogue {
    let mut catalogue = Catalogue::default();
    let mut entry = Entry::default();
    let mut field = Field::None;

    for line in text.lines() {
        let trimmed = line.trim();

        // A continuation: another quoted string belonging to the field above.
        // An entry is therefore only finished when the *next* one starts, which
        // is why closing it happens on `msgctxt`, `msgid` and a comment rather
        // than on `msgstr`.
        if trimmed.starts_with('"') {
            let value = unquote(trimmed);
            match field {
                Field::Context => entry.context.push_str(&value),
                Field::Source => entry.source.push_str(&value),
                Field::Target => entry.target.push_str(&value),
                Field::None => {}
            }
            continue;
        }

        let opens_an_entry = trimmed.starts_with('#')
            || trimmed.starts_with("msgctxt ")
            || trimmed.starts_with("msgid ");

        if opens_an_entry && field == Field::Target {
            close(&mut catalogue, std::mem::take(&mut entry));
            field = Field::None;
        }

        // Anything unrecognised — a blank line, a `#, fuzzy` flag, a line the
        // author fat-fingered — leaves the open field alone rather than
        // resetting it. Resetting would orphan the entry above it, and PO puts a
        // blank line between every pair of entries.
        if let Some(note) = trimmed.strip_prefix("#.") {
            entry.notes.push(note.trim().to_string());
        } else if let Some(reference) = trimmed.strip_prefix("#:") {
            entry.reference = reference.trim().to_string();
        } else if let Some(value) = trimmed.strip_prefix("msgctxt ") {
            entry.context = unquote(value);
            field = Field::Context;
        } else if let Some(value) = trimmed.strip_prefix("msgid ") {
            entry.source = unquote(value);
            field = Field::Source;
        } else if let Some(value) = trimmed.strip_prefix("msgstr ") {
            entry.target = unquote(value);
            field = Field::Target;
        }
    }

    if field == Field::Target {
        close(&mut catalogue, entry);
    }

    catalogue
}

/// Files a finished entry, or reads it as the header.
///
/// The header is the entry whose msgid is empty, which is Gettext's own
/// convention: its translation is a block of `Name: value` metadata rather than
/// anything that appears on a slide.
fn close(catalogue: &mut Catalogue, entry: Entry) {
    if entry.source.is_empty() && entry.context.is_empty() {
        read_header(catalogue, &entry.target);
        return;
    }

    catalogue.entries.push(entry);
}

/// Which field a bare `"…"` continuation line belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    None,
    Context,
    Source,
    Target,
}

fn read_header(catalogue: &mut Catalogue, line: &str) {
    for field in line.split('\n') {
        if let Some(value) = field.trim().strip_prefix("Language:") {
            catalogue.lang = value.trim().to_string();
        } else if let Some(value) = field.trim().strip_prefix("X-Slidx-Deck:") {
            catalogue.deck = value.trim().to_string();
        }
    }
}

/// A keyword and its value, split across lines the way PO splits them.
///
/// A multi-line string is written as an empty first string and one line per
/// line of text, which is what makes a paragraph readable in the file and its
/// change readable in a diff.
fn quoted(keyword: &str, value: &str) -> String {
    if !value.contains('\n') {
        return format!("{keyword} \"{}\"\n", escape(value));
    }

    let mut out = format!("{keyword} \"\"\n");
    let lines: Vec<&str> = value.split('\n').collect();

    for (index, line) in lines.iter().enumerate() {
        let terminator = if index + 1 < lines.len() { "\\n" } else { "" };
        out.push_str(&format!("\"{}{terminator}\"\n", escape(line)));
    }

    out
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"").replace('\t', "\\t")
}

/// The text inside a `"…"`, unescaped.
fn unquote(value: &str) -> String {
    let inner =
        value.trim().strip_prefix('"').and_then(|rest| rest.strip_suffix('"')).unwrap_or("");
    let mut out = String::with_capacity(inner.len());
    let mut escaped = false;

    for character in inner.chars() {
        match (escaped, character) {
            (true, 'n') => out.push('\n'),
            (true, 't') => out.push('\t'),
            (true, other) => out.push(other),
            (false, '\\') => {
                escaped = true;
                continue;
            }
            (false, other) => out.push(other),
        }
        escaped = false;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(context: &str, source: &str, target: &str) -> Entry {
        Entry {
            context: context.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            notes: vec!["Body of slide `one`.".to_string()],
            reference: "one:5".to_string(),
        }
    }

    fn catalogue(entries: &[Entry]) -> Catalogue {
        Catalogue { lang: "ja".to_string(), deck: "slides".to_string(), entries: entries.to_vec() }
    }

    fn round_trip(cat: &Catalogue) -> Catalogue {
        parse(&render(cat))
    }

    #[test]
    fn a_catalogue_survives_being_written_and_read_back() {
        let cat = catalogue(&[
            entry("one/heading", "Fast Decks", "速いデッキ"),
            entry("one/body/1", "A framework for the whole life of a talk.", ""),
        ]);

        assert_eq!(round_trip(&cat), cat);
    }

    #[test]
    fn the_language_and_the_deck_travel_in_the_header_where_po_tools_look() {
        let cat = round_trip(&catalogue(&[entry("a", "b", "c")]));

        assert_eq!(cat.lang, "ja");
        assert_eq!(cat.deck, "slides");
    }

    #[test]
    fn a_multi_line_string_is_written_one_line_per_line_so_a_diff_is_readable() {
        // The reason PO was chosen over XML. A change to the second line of a
        // paragraph shows up as a change to the second line.
        let po = render(&catalogue(&[entry("a", "first line\nsecond line", "")]));

        assert!(po.contains("msgid \"\"\n\"first line\\n\"\n\"second line\"\n"), "{po}");
    }

    #[test]
    fn a_multi_line_string_reads_back_with_its_newlines_intact() {
        let cat = catalogue(&[entry("a", "first\nsecond\nthird", "一\n二")]);
        assert_eq!(round_trip(&cat), cat);
    }

    #[test]
    fn quotes_and_backslashes_in_prose_survive_the_round_trip() {
        let cat = catalogue(&[entry("a", r#"He said "no" \ twice"#, "")]);
        assert_eq!(round_trip(&cat), cat);
    }

    #[test]
    fn placeholders_are_written_as_they_are_because_po_leaves_them_alone() {
        let po = render(&catalogue(&[entry("a", "Latency dropped to [120ms]%1.", "")]));

        assert!(po.contains("msgid \"Latency dropped to [120ms]%1.\""), "{po}");
    }

    #[test]
    fn the_header_explains_the_placeholders_to_whoever_opens_the_file() {
        // The file is the whole interface to a translator. A convention nobody
        // wrote down is a convention somebody breaks.
        let po = render(&catalogue(&[]));

        assert!(po.contains("%1"), "{po}");
        assert!(po.contains("must not be translated"), "{po}");
        assert!(po.contains("empty msgstr"), "{po}");
    }

    #[test]
    fn a_translator_written_file_without_our_comments_still_reads() {
        // What comes back from a translation tool is not what we wrote out.
        let po = "msgid \"\"\nmsgstr \"Language: de\\n\"\n\n\
                  msgctxt \"one/heading\"\nmsgid \"Fast Decks\"\nmsgstr \"Schnelle Decks\"\n";
        let cat = parse(po);

        assert_eq!(cat.lang, "de");
        assert_eq!(cat.entries.len(), 1);
        assert_eq!(cat.entries[0].target, "Schnelle Decks");
    }

    #[test]
    fn an_entry_with_no_context_is_still_read_as_an_entry() {
        // Not everything that produces PO writes msgctxt.
        let cat = parse("msgid \"Fast Decks\"\nmsgstr \"速いデッキ\"\n");

        assert_eq!(cat.entries.len(), 1);
        assert_eq!(cat.entries[0].source, "Fast Decks");
    }

    #[test]
    fn a_fuzzy_flag_is_kept_as_a_comment_rather_than_dropped_silently() {
        // Round-tripping it is the point: dropping it would tell the next
        // reviewer a translation had been approved when it had not.
        let cat = parse("#, fuzzy\nmsgctxt \"a\"\nmsgid \"x\"\nmsgstr \"y\"\n");

        assert_eq!(cat.entries[0].target, "y");
    }

    #[test]
    fn a_malformed_line_is_skipped_rather_than_failing_the_whole_catalogue() {
        // Catalogues are hand-edited, sometimes on the morning of a talk. A
        // parse error that dropped the translation would be the worst answer.
        let cat = parse("nonsense\nmsgctxt \"a\"\nmsgid \"x\"\nmsgstr \"y\"\n???\n");

        assert_eq!(cat.entries.len(), 1);
    }

    #[test]
    fn an_empty_catalogue_still_carries_a_header_a_tool_can_read() {
        let po = render(&Catalogue { lang: "fr".into(), ..Catalogue::default() });

        assert!(po.contains("\"Language: fr\\n\""), "{po}");
        assert!(po.contains("charset=UTF-8"), "{po}");
    }
}
