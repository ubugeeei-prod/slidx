//! Putting a catalogue back into the deck it came out of.
//!
//! Two things happen here and the second is the one that took the work.
//!
//! The prose is a set of byte-range splices, built with `slidx_edit`'s own
//! builder so there is one implementation of what an operation is allowed to
//! touch. A segment with no translation plans no splice at all, which is why a
//! catalogue nobody has filled in yields the file back byte for byte.
//!
//! Then the ids. A slide's id is a slug of its heading, so translating headings
//! moves slides — and not only the ones that were translated: two slides titled
//! "Demo" resolve to `demo` and `demo-2`, so translating the *first* one frees
//! `demo` and silently renames the second. Every deep link and every QR code
//! into the deck addresses the old ids. So the translated text is parsed, its
//! ids are compared against the original's, and every id that moved is pinned
//! back with `id:` in that slide's frontmatter — which means no noise at all
//! when nothing moved, and a pin exactly where one is needed.

use slidx_core::{find_marks, parse_deck, DeckParseOptions};
use slidx_edit::{Edit, EditBuilder, EditOp, SlideRef};

use crate::catalogue::Catalogue;
use crate::extract;
use crate::protect::restore;
use crate::segment::{Segment, SegmentKind};

/// Something slidx would not do, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// A translation left out a placeholder, so the markup it stood for would
    /// have been dropped with it.
    DroppedPlaceholder { context: String, placeholder: usize, stood_for: String },
    /// A translation kept every placeholder and still stopped a mark being one.
    ///
    /// `[3.2x faster]%1` becomes a mark again only while the bracket and the
    /// braces stay next to each other. A translation that put a word between
    /// them leaves the key visible in the file and invisible to the parser,
    /// which is the same silent failure by another route.
    LostMark { context: String, key: String },
    /// A catalogue entry the deck has no segment for any more.
    Stale { context: String },
    /// The deck's ids could not be pinned, so a translated heading moves a slide.
    CouldNotPin,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DroppedPlaceholder { context, placeholder, stood_for } => write!(
                out,
                "{context}: the translation left out %{placeholder}, which stands for \
                 `{stood_for}`. Put it back — slidx will not write this string without it."
            ),
            Self::LostMark { context, key } => write!(
                out,
                "{context}: the translation separated `[…]` from the placeholder after it, so \
                 `#{key}` stops being a mark. Keep the placeholder against the closing bracket."
            ),
            Self::Stale { context } => write!(
                out,
                "{context}: the deck no longer has this string. Re-run `slidx i18n extract`."
            ),
            Self::CouldNotPin => write!(
                out,
                "the translated deck's slide ids could not be pinned, so a link into it may not \
                 resolve. Give the slides an `id:` of their own before translating."
            ),
        }
    }
}

/// What applying a catalogue would do, worked out without doing it.
#[derive(Debug, Clone)]
pub struct Plan {
    /// The translation, as splices into the source it was planned against.
    pub edit: Edit,
    /// Frontmatter keys the translated deck has to carry: the pinned ids, the
    /// language, and the deck it came from.
    ///
    /// Applied after the prose rather than folded into the same edit, because
    /// which ids move can only be known by parsing the translated text.
    fields: Vec<EditOp>,
    options: DeckParseOptions,
    pub problems: Vec<Problem>,
    /// Segments a translation was found and used for.
    pub translated: usize,
    /// Segments the catalogue had no translation for. Those stay in the deck's
    /// original language, which is the only safe direction.
    pub untranslated: usize,
}

impl Plan {
    /// The source with the translation and the pins written in.
    pub fn apply(&self, source: &str) -> String {
        self.apply_tracking(source, &mut [])
    }

    /// The same, reporting where each of `marks` ended up.
    ///
    /// A deck is usually stored one file per slide and translated as one joined
    /// source, so whoever writes it back has to know where each file's bytes
    /// went. Tracked through the edit rather than recomputed from the output,
    /// because recomputing would be a second answer to where the seams are — and
    /// a wrong one the moment a translated heading changes a slide's length.
    ///
    /// A mark does not move for a change that begins exactly on it, so an `id:`
    /// block inserted at a file's first byte belongs to that file rather than to
    /// the separator above it.
    pub fn apply_tracking(&self, source: &str, marks: &mut [usize]) -> String {
        let mut out = self.edit.apply(source);
        shift(&self.edit, marks);

        for op in &self.fields {
            let Ok(edit) = slidx_edit::plan(&out, &self.options, op) else { continue };
            out = edit.apply(&out);
            shift(&edit, marks);
        }

        out
    }

    /// True when the catalogue asks for nothing at all.
    pub fn is_empty(&self) -> bool {
        self.edit.is_empty() && self.fields.is_empty()
    }

    /// Slide ids this plan has to pin so a published URL keeps working.
    pub fn pinned_ids(&self) -> Vec<&str> {
        self.fields
            .iter()
            .filter_map(|op| match op {
                EditOp::SetField { key, value, .. } if key == "id" => value.as_str(),
                _ => None,
            })
            .collect()
    }
}

/// Works out what a catalogue changes, without changing it.
pub fn plan(source: &str, options: &DeckParseOptions, catalogue: &Catalogue) -> Plan {
    let segments = extract::segments(source, options);
    let mut builder = EditBuilder::new(source);
    let mut problems = Vec::new();
    let mut translated = 0usize;
    let mut untranslated = 0usize;

    for segment in &segments {
        let Some(entry) = catalogue.find(&segment.context) else {
            untranslated += 1;
            continue;
        };
        if entry.is_untranslated() {
            untranslated += 1;
            continue;
        }

        let text = match restore(&entry.target, &segment.protected) {
            Ok(text) => text,
            Err(placeholder) => {
                problems.push(Problem::DroppedPlaceholder {
                    context: segment.context.clone(),
                    placeholder,
                    stood_for: segment.protected[placeholder - 1].clone(),
                });
                untranslated += 1;
                continue;
            }
        };

        if let Some(key) = mark_lost(segment.span.slice(source), &text) {
            problems.push(Problem::LostMark { context: segment.context.clone(), key });
            untranslated += 1;
            continue;
        }

        builder.replace(segment.span, written(segment, &text));
        translated += 1;
    }

    for entry in &catalogue.entries {
        if !segments.iter().any(|segment| segment.context == entry.context) {
            problems.push(Problem::Stale { context: entry.context.clone() });
        }
    }

    let edit = builder.build();
    let translated_text = edit.apply(source);
    let wanted = ids(source, options);

    // Frontmatter written into a deck that had none moves where the parser
    // thinks the first slide begins, on a source whose body opens with a
    // separator. These fields exist only to keep the deck's addresses, so a set
    // of them that moves one is not written: the deck-level keys go first,
    // because they are the ones that can create a block at byte zero, and if
    // that is still not enough the author is told rather than shipped a deck at
    // addresses nobody published.
    let full = fields_for(source, &translated_text, options, catalogue);
    let pins: Vec<EditOp> = full.iter().filter(|op| is_pin(op)).cloned().collect();

    let (fields, pinned) = [full, pins, Vec::new()]
        .into_iter()
        .find_map(|candidate| {
            let out = replay(&translated_text, options, &candidate);
            (ids(&out, options) == wanted).then_some((candidate, true))
        })
        .unwrap_or_else(|| (Vec::new(), false));

    if !pinned {
        problems.push(Problem::CouldNotPin);
    }

    Plan { edit, fields, options: options.clone(), problems, translated, untranslated }
}

/// Every slide id, in order.
fn ids(source: &str, options: &DeckParseOptions) -> Vec<String> {
    parse_deck(source, options).slides.into_iter().map(|slide| slide.id).collect()
}

fn is_pin(op: &EditOp) -> bool {
    matches!(op, EditOp::SetField { key, .. } if key == "id")
}

/// The text with a run of field operations applied, exactly as [`Plan::apply`]
/// will apply them.
fn replay(source: &str, options: &DeckParseOptions, fields: &[EditOp]) -> String {
    fields.iter().fold(source.to_string(), |text, op| {
        slidx_edit::apply(&text, options, op).unwrap_or(text)
    })
}

/// A mark key the source declares and the translation no longer does.
///
/// Restoring the placeholders puts the exact bytes of `{#latency}` back, and
/// that is still not enough: a mark is `[text]{…}` with nothing between them, so
/// a translation that wrote a word after the closing bracket leaves a key that
/// is in the file and not in the deck. Checked against the parser rather than
/// against a rule of thumb, because the parser is what decides.
fn mark_lost(source: &str, translated: &str) -> Option<String> {
    let keys = |text: &str| -> Vec<String> {
        find_marks(text).into_iter().filter_map(|found| found.mark.key).collect()
    };

    let after = keys(translated);
    keys(source).into_iter().find(|key| !after.contains(key))
}

/// Moves every mark to where an edit puts it.
fn shift(edit: &Edit, marks: &mut [usize]) {
    for mark in marks.iter_mut() {
        let drift: isize = edit
            .splices()
            .iter()
            .take_while(|splice| splice.span.start < *mark)
            .map(|splice| splice.text.len() as isize - splice.span.len() as isize)
            .sum();

        *mark = (*mark as isize + drift).max(0) as usize;
    }
}

/// How a translated string is written back into the file.
///
/// A body block goes in as it is. A frontmatter value has to become YAML again,
/// through the same quoting `slidx_edit` uses — a Japanese title containing a
/// colon would otherwise break the block it lives in, and the deck would lose
/// its title rather than gain a translated one.
fn written(segment: &Segment, text: &str) -> String {
    match segment.kind {
        SegmentKind::Meta(_) => {
            format!(" {}", slidx_edit::frontmatter::scalar(&serde_json::Value::from(text)))
        }
        _ => text.to_string(),
    }
}

/// The frontmatter keys a translated deck has to carry.
///
/// Every id that the translation moved, pinned back, plus what the deck is and
/// what it came from. Nothing at all for a deck no string of which was
/// translated: it is still in the language it was written in, and saying `lang:
/// ja` over English would be a lie that a screen reader acts on. That is also
/// what makes applying an empty catalogue return the file byte for byte.
///
/// Nothing either for a deck whose ids all survived, because a pin nobody needs
/// is a line in a diff nobody can act on.
fn fields_for(
    source: &str,
    translated: &str,
    options: &DeckParseOptions,
    catalogue: &Catalogue,
) -> Vec<EditOp> {
    let mut fields = Vec::new();

    if translated == source {
        return fields;
    }

    let before = parse_deck(source, options);
    let after = parse_deck(translated, options);

    for (index, slide) in before.slides.iter().enumerate() {
        let moved = after.slides.get(index).is_some_and(|other| other.id != slide.id);

        if moved {
            fields.push(EditOp::SetField {
                slide: SlideRef::Index(index),
                key: "id".to_string(),
                value: serde_json::Value::from(slide.id.as_str()),
            });
        }
    }

    if !catalogue.lang.is_empty() {
        fields.push(field(0, "lang", &catalogue.lang));
    }
    if !catalogue.deck.is_empty() {
        fields.push(field(0, "translationOf", &catalogue.deck));
    }

    fields
}

fn field(slide: usize, key: &str, value: &str) -> EditOp {
    EditOp::SetField {
        slide: SlideRef::Index(slide),
        key: key.to_string(),
        value: serde_json::Value::from(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogue::Entry;

    fn options() -> DeckParseOptions {
        DeckParseOptions::default()
    }

    /// A catalogue translating exactly the entries it is given.
    fn catalogue(pairs: &[(&str, &str, &str)]) -> Catalogue {
        Catalogue {
            lang: String::new(),
            deck: String::new(),
            entries: pairs
                .iter()
                .map(|(context, source, target)| Entry {
                    context: (*context).to_string(),
                    source: (*source).to_string(),
                    target: (*target).to_string(),
                    ..Entry::default()
                })
                .collect(),
        }
    }

    fn applied(source: &str, pairs: &[(&str, &str, &str)]) -> String {
        plan(source, &options(), &catalogue(pairs)).apply(source)
    }

    #[test]
    fn a_catalogue_that_translates_nothing_leaves_the_file_byte_identical() {
        // Not "produces the same text" — plans no splice at all, so the bytes
        // are never read and cannot change. It is what makes an unfinished
        // translation safe to apply.
        let source = "---\ntitle: Fast Decks\n---\n\n#   Fast Decks\n\n* one\n* two\n";
        let planned = plan(source, &options(), &Catalogue::default());

        assert!(planned.is_empty());
        assert_eq!(planned.apply(source), source);
    }

    #[test]
    fn an_untranslated_entry_leaves_its_string_in_the_original_language() {
        // A blank slide is worse than an English one in front of a Japanese
        // audience.
        let source = "# One\n\nBody.\n";
        assert_eq!(applied(source, &[("one/heading", "One", "")]), source);
    }

    #[test]
    fn the_authors_formatting_around_a_translated_line_survives_untouched() {
        // The reason this is a splice. Re-serialising would regularise the three
        // spaces and the `*` bullets, and the translation diff would be about
        // style. The `id:` block is the pin the translated heading needs.
        let source = "#   Fast Decks\n\n* one\n* two\n";
        let out = applied(source, &[("fast-decks/heading", "Fast Decks", "速いデッキ")]);

        assert_eq!(out, "---\nid: fast-decks\n---\n\n#   速いデッキ\n\n* one\n* two\n");
    }

    #[test]
    fn a_translated_heading_pins_the_id_the_original_deck_published() {
        // The failure that is easy to miss: every deep link and every QR code
        // into this deck addresses `fast-decks`.
        let source = "# Fast Decks\n\nBody.\n";
        let out = applied(source, &[("fast-decks/heading", "Fast Decks", "速いデッキ")]);

        assert!(out.contains("id: fast-decks"), "{out}");
        assert_eq!(parse_deck(&out, &options()).slides[0].id, "fast-decks");
    }

    #[test]
    fn translating_one_of_two_slides_that_shared_a_title_does_not_rename_the_other() {
        // `demo` and `demo-2`. Translating the first frees `demo`, and the
        // second silently takes it — a slide that was never touched changing
        // its URL.
        let source = "# Demo\n\n---\n\n# Demo\n";
        let out = applied(source, &[("demo/heading", "Demo", "デモ")]);
        let after = parse_deck(&out, &options());

        assert_eq!(after.slides[0].id, "demo");
        assert_eq!(after.slides[1].id, "demo-2");
    }

    #[test]
    fn a_deck_whose_ids_all_survive_gains_no_pins_at_all() {
        // A pin nobody needs is a line in a diff nobody can act on.
        let source = "# One\n\nBody.\n";
        let out = applied(source, &[("one/body/1", "Body.", "本文。")]);

        assert!(!out.contains("id:"), "{out}");
    }

    #[test]
    fn a_translated_title_is_re_quoted_so_it_stays_one_yaml_value() {
        // A Japanese title with a colon in it would end the value early and the
        // deck would lose its title instead of gaining a translated one.
        let source = "---\ntitle: Fast Decks\n---\n\n# One\n";
        let out = applied(source, &[("deck/title", "Fast Decks", "速さ: デッキの話")]);

        assert_eq!(
            parse_deck(&out, &options()).meta.title.as_deref(),
            Some("速さ: デッキの話")
        );
    }

    #[test]
    fn a_translation_that_dropped_a_mark_key_is_refused_and_reported() {
        // Writing it would leave a deck that renders perfectly with the
        // animation gone. Nothing else in the pipeline could notice.
        let source = "Latency dropped to [120ms]{#latency}.\n";
        let cat = catalogue(&[(
            "slide-1/body/1",
            "Latency dropped to [120ms]%1.",
            "レイテンシが下がりました。",
        )]);
        let planned = plan(source, &options(), &cat);

        assert_eq!(planned.apply(source), source, "the source is left alone");
        assert!(matches!(
            planned.problems.first(),
            Some(Problem::DroppedPlaceholder { placeholder: 1, .. })
        ));
    }

    #[test]
    fn a_problem_says_what_the_placeholder_stood_for_so_it_can_be_put_back() {
        let source = "Latency dropped to [120ms]{#latency}.\n";
        let cat = catalogue(&[("slide-1/body/1", "Latency dropped to [120ms]%1.", "下がった。")]);

        let message = plan(source, &options(), &cat).problems[0].to_string();

        assert!(message.contains("{#latency}"), "{message}");
        assert!(message.contains("%1"), "{message}");
    }

    #[test]
    fn a_translation_that_kept_the_key_and_still_stopped_it_being_a_mark_is_refused() {
        // The second way to lose an animation, and it passes the placeholder
        // check: `[3.2x faster]%1` is a mark only while the bracket and the
        // braces stay next to each other. A word between them leaves `#result`
        // in the file and out of the deck.
        let source = "The result was [3.2x faster]{#result}.\n";
        let cat = catalogue(&[(
            "slide-1/body/1",
            "The result was [3.2x faster]%1.",
            "結果は [3.2倍速く] という %1 でした。",
        )]);
        let planned = plan(source, &options(), &cat);

        assert_eq!(planned.apply(source), source, "the source is left alone");
        assert!(matches!(planned.problems.first(), Some(Problem::LostMark { .. })));
    }

    #[test]
    fn a_lost_mark_is_reported_by_the_key_that_would_have_gone_missing() {
        let source = "The result was [3.2x faster]{#result}.\n";
        let cat = catalogue(&[(
            "slide-1/body/1",
            "The result was [3.2x faster]%1.",
            "結果は [3.2倍速く] の %1 です。",
        )]);

        assert!(plan(source, &options(), &cat).problems[0].to_string().contains("#result"));
    }

    #[test]
    fn a_pin_that_would_change_the_deck_is_withheld_and_reported() {
        // A deck with no frontmatter whose body opens on a separator cannot be
        // given a block without moving where its first slide starts. Writing the
        // pin anyway would trade one broken address for a broken deck.
        let source = "\n---\n# 見出し\n";
        let cat = catalogue(&[("見出し/heading", "見出し", "Heading")]);
        let planned = plan(source, &options(), &cat);

        assert!(planned.problems.contains(&Problem::CouldNotPin));
        assert!(!planned.apply(source).contains("id:"));
    }

    #[test]
    fn an_entry_for_a_string_the_deck_no_longer_has_is_reported_as_stale() {
        // The catalogue outlives the deck. Silence here would mean a translator
        // never learns that a week of work is addressed at a deleted slide.
        let planned = plan("# One\n", &options(), &catalogue(&[("gone/body/1", "x", "y")]));

        assert!(planned.problems.iter().any(|p| matches!(p, Problem::Stale { .. })));
    }

    #[test]
    fn a_plan_counts_what_it_did_and_what_is_still_owed() {
        let source = "# One\n\nBody.\n";
        let planned = plan(source, &options(), &catalogue(&[("one/heading", "One", "一")]));

        assert_eq!(planned.translated, 1);
        assert_eq!(planned.untranslated, 1);
    }

    #[test]
    fn the_translated_deck_says_which_language_it_is_and_what_it_came_from() {
        // Without both, nothing downstream can tell that two decks are the same
        // talk, and a screen reader picks an English voice for Japanese.
        let source = "---\ntitle: T\n---\n\n# One\n";
        let mut cat = catalogue(&[("one/heading", "One", "一")]);
        cat.lang = "ja".into();
        cat.deck = "slides".into();
        let out = plan(source, &options(), &cat).apply(source);

        let meta = parse_deck(&out, &options()).meta;
        assert_eq!(meta.lang.as_deref(), Some("ja"));
        assert_eq!(meta.translation_of.as_deref(), Some("slides"));
    }

    #[test]
    fn a_deck_nothing_was_translated_in_does_not_claim_to_be_in_another_language() {
        // `lang: ja` over English words is a lie a screen reader acts on. It is
        // also what would stop an empty catalogue returning the file untouched.
        let source = "---\ntitle: T\n---\n\n# One\n";
        let cat = Catalogue { lang: "ja".into(), deck: "slides".into(), entries: Vec::new() };

        assert_eq!(plan(source, &options(), &cat).apply(source), source);
    }

    #[test]
    fn notes_are_translated_alongside_the_slide_they_belong_to() {
        let source = "# One\n\n<!-- notes: open with the outcome -->\n";
        let out = applied(
            source,
            &[
                ("one/heading", "One", "一"),
                ("one/notes/1", "open with the outcome", "結果から始める"),
            ],
        );

        assert!(out.contains("<!-- notes: 結果から始める -->"), "{out}");
    }

    #[test]
    fn a_deck_written_with_crlf_keeps_its_line_endings() {
        // Splicing the bytes the author saved means never converting them. A
        // `^M` in the diff would be in the one place nobody thinks to look.
        let source = "# One\r\n\r\nBody.\r\n";
        let out = applied(source, &[("one/heading", "One", "一")]);

        assert_eq!(out, "---\r\nid: one\r\n---\r\n\r\n# 一\r\n\r\nBody.\r\n");
        assert!(!out.contains("\n\n"), "not one bare newline anywhere");
    }
}
