//! The file a translator works in, and a reviewer reads.
//!
//! One entry per translatable segment: where it came from, what it says, what a
//! translator needs to know about it, and the translation. An empty translation
//! means nobody has written one yet, which is the state that makes applying a
//! half-finished catalogue safe — the untranslated half of a deck stays in the
//! language it was written in rather than becoming blank slides.
//!
//! The serialised form is Gettext PO; see [`po`] for why that rather than
//! something invented here.

mod po;

use crate::segment::Segment;

/// One string, and what is known about it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    /// The segment's stable address. `msgctxt`.
    ///
    /// What makes two slides that both say "Demo" two entries. Without it a
    /// catalogue could hold only one translation for both, and a deck that says
    /// the same word twice in two senses is ordinary.
    pub context: String,
    /// The text as the deck says it, placeholders and all. `msgid`.
    pub source: String,
    /// The translation, empty until somebody writes one. `msgstr`.
    pub target: String,
    /// What a translator is told beyond the string. `#.` comments.
    pub notes: Vec<String>,
    /// Where it came from, as `path:line`. `#:` reference.
    pub reference: String,
}

impl Entry {
    /// True when nobody has translated this yet.
    pub fn is_untranslated(&self) -> bool {
        self.target.trim().is_empty()
    }
}

/// Every entry for one language.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalogue {
    /// BCP 47 tag this catalogue translates into.
    pub lang: String,
    /// The deck the entries came from, as the author named it on the command
    /// line. Carried so `apply` can say when it is being pointed at a different
    /// deck than the one that was extracted.
    pub deck: String,
    pub entries: Vec<Entry>,
}

impl Catalogue {
    /// A fresh catalogue with an entry per segment and no translations.
    pub fn of(lang: &str, segments: Vec<Segment>) -> Self {
        let entries = segments
            .into_iter()
            .map(|segment| Entry {
                reference: format!("{}:{}", segment.slide, segment.line),
                notes: segment.notes(),
                context: segment.context,
                source: segment.text,
                target: String::new(),
            })
            .collect();

        Self { lang: lang.to_string(), deck: String::new(), entries }
    }

    /// The translation for one segment, if there is one.
    pub fn find(&self, context: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.context == context)
    }

    /// Copies translations from an older catalogue onto this one.
    ///
    /// Matched on context **and** source text together. Context alone would
    /// carry a translation of the old words onto new ones and call the slide
    /// done; source alone would move a translation onto a different slide that
    /// happens to say the same sentence. Requiring both means an edited line
    /// comes back untranslated, which is the answer that gets noticed.
    pub fn carry_over(&mut self, previous: &Self) {
        for entry in &mut self.entries {
            let Some(old) = previous.find(&entry.context) else { continue };

            if old.source == entry.source {
                entry.target = old.target.clone();
            }
        }
    }

    /// How many entries still have nobody's translation in them.
    pub fn untranslated(&self) -> usize {
        self.entries.iter().filter(|entry| entry.is_untranslated()).count()
    }

    /// The catalogue as the Gettext PO text a translator's tool opens.
    pub fn to_po(&self) -> String {
        po::render(self)
    }

    /// A catalogue read back from PO text.
    ///
    /// Total: a line it does not understand is skipped rather than refused,
    /// because a catalogue is hand-edited — sometimes on the morning of a talk —
    /// and losing the whole translation to one bad line is the worst answer.
    pub fn from_po(text: &str) -> Self {
        po::parse(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(context: &str, source: &str, target: &str) -> Entry {
        Entry {
            context: context.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            ..Entry::default()
        }
    }

    fn catalogue(entries: &[Entry]) -> Catalogue {
        Catalogue { lang: "ja".to_string(), deck: String::new(), entries: entries.to_vec() }
    }

    #[test]
    fn a_translation_carries_over_when_the_address_and_the_words_both_match() {
        // The case that makes a catalogue usable twice: re-extracting after
        // fixing a typo on slide one must not throw away the rest.
        let mut fresh = catalogue(&[entry("one/heading", "Fast Decks", "")]);
        fresh.carry_over(&catalogue(&[entry("one/heading", "Fast Decks", "速いデッキ")]));

        assert_eq!(fresh.entries[0].target, "速いデッキ");
    }

    #[test]
    fn an_edited_line_comes_back_untranslated_rather_than_stale() {
        // A translation of words that are no longer there is worse than none: it
        // reports the slide as done and says the wrong thing on stage.
        let mut fresh = catalogue(&[entry("one/heading", "Faster Decks", "")]);
        fresh.carry_over(&catalogue(&[entry("one/heading", "Fast Decks", "速いデッキ")]));

        assert!(fresh.entries[0].is_untranslated());
    }

    #[test]
    fn the_same_sentence_on_two_slides_does_not_share_one_translation() {
        // Addressed by context, so a word that means two things in two places
        // can be translated two ways.
        let mut fresh = catalogue(&[entry("two/body/1", "Demo", "")]);
        fresh.carry_over(&catalogue(&[entry("one/body/1", "Demo", "デモ")]));

        assert!(fresh.entries[0].is_untranslated());
    }

    #[test]
    fn a_catalogue_reports_how_much_of_it_is_still_nobodys_work() {
        let cat = catalogue(&[entry("a", "x", "訳"), entry("b", "y", ""), entry("c", "z", "  ")]);

        assert_eq!(cat.untranslated(), 2, "whitespace is not a translation");
    }
}
