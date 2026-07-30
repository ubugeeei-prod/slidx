//! # slidx i18n
//!
//! Extracting a deck's prose, and splicing a translation back into it.
//!
//! ## Why this exists
//!
//! A talk given twice in two languages is one talk, and today it is two forks
//! that drift. Finding the prose in a Markdown deck is the easy half. This
//! crate exists for the other half: **everything in a deck that is not prose,
//! and what breaks when a translator moves it.** Every item below is something
//! a naive pass over the Markdown would rewrite, and most of them fail
//! *silently* — the deck still builds, still renders, and is wrong.
//!
//! - **Mark keys.** In `Latency dropped to [120ms]{#latency}`, `120ms` is
//!   content and `#latency` is an address. A `steps:` entry points at that key,
//!   so a translated key leaves a deck that renders perfectly with the
//!   animation gone and nothing to say why.
//! - **Mark classes and property names.** `.accent`, `color=danger`. A closed
//!   vocabulary the theme owns and `steps: - set: { target, color }` names.
//! - **Fenced code, and the fence's own info line.** Including the comments
//!   inside it: a translated code comment no longer matches the recording of
//!   the talk. The info line carries the `.share` key, which is a file name and
//!   a URL in the deck's own output.
//! - **Inline code.** `` `contrast/projector` `` is a diagnostic code, not a
//!   phrase. Same for every API name a talk about code is full of.
//! - **URLs and image paths.** A link's text is prose and its destination is
//!   not. Translating `./diagram.png` produces a slide with a broken image and
//!   a build that still succeeds.
//! - **Frontmatter keys.** `title:` is a key and its value is prose. A
//!   translated key is a deck with no title. Only `title` and `description`
//!   hold prose at all; `theme`, `layout`, `transition`, `aspect`, `autoSteps`,
//!   `steps`, `budget`, `demo`, `url` and `repo` are vocabulary or addresses,
//!   and a key slidx has never heard of belongs to a theme — so this is an
//!   allow-list rather than a deny-list.
//! - **Step markers.** `<!-- step -->` is a position in a pipeline.
//! - **HTML tags and their attributes.** A slide opting into an island declares
//!   it in markup, and the tag names and attribute values are the declaration.
//!   The text between two tags is prose and is offered.
//! - **Slide ids** — the one that is easy to miss and worse than the rest.
//!   A slide's id is a slug of its heading, so **translating a heading moves
//!   the slide**: every deep link a speaker pasted into a chat, every QR code
//!   printed on a handout, and every anchor in the published deck addresses the
//!   old one. So a translated slide is written with `id:` pinned to the id the
//!   original deck derived, and [`slidx_core`] honours that pin ahead of the
//!   slug. It is not pinned by guessing which headings changed: two slides
//!   titled "Demo" resolve to `demo` and `demo-2`, so translating the *first*
//!   frees `demo` and silently renames the second. The translated text is
//!   parsed and its ids are compared against the original's, which catches
//!   that. `every_slide_id_survives_a_full_translation_or_the_author_is_told_it_did_not`
//!   in `tests/translation_properties.rs` is the test that holds it.
//!
//! ## How the protection works
//!
//! Not by asking a translator to be careful. Every one of those regions is
//! replaced by a numbered placeholder before the text ever reaches a catalogue,
//! so the key, the URL and the code span are physically absent from the file a
//! translator edits and cannot be retyped wrongly. Placeholders may be
//! *reordered* — Japanese puts the verb last — and [`apply`] refuses an entry
//! that drops one rather than silently dropping the markup with it.
//!
//! ## Why it is a splice and not a re-serialisation
//!
//! The same reason `slidx_edit` is: parsing a deck, translating the model and
//! writing it back out regularises the author's blank lines, their `*` bullets
//! and their hand-wrapped paragraphs. A translation diff has to be reviewable
//! by someone who reads the target language and not the tool, so applying a
//! catalogue is a set of **byte-range splices** into the source the author
//! saved, built with `slidx_edit`'s own [`slidx_edit::EditBuilder`]. A
//! catalogue that translates nothing therefore produces the file back byte for
//! byte, which is asserted rather than hoped for.
//!
//! ## Why slidx does not translate
//!
//! There is no machine translation here and no network call, for the same
//! reason `slidx publish` holds no token: producing a translation is the
//! author's job, with whatever tool or person they choose, and a build that
//! called a translation service would be a build that is neither deterministic
//! nor offline. The catalogue *is* the hook — a Gettext PO file, which every
//! translation tool already reads — and running one over it is a thing the
//! author does deliberately, between `extract` and `apply`.
//!
//! ## Why PO
//!
//! Not invented here. PO is line-oriented, so a translation change arrives in a
//! pull request as a diff a reviewer can read; it has a first-class place for
//! the context a translator needs (`#.`) and for where the string came from
//! (`#:`); and `msgctxt` distinguishes two slides that both say "Demo", which a
//! deck needs and a bare key-value format cannot express. XLIFF is the other
//! standard and is XML — a one-word change becomes a diff inside a tree.
//!
//! ## What has to move together, and does not
//!
//! Notes are extracted with the slide they belong to, because a translated
//! slide with untranslated notes is worse than neither. Two things deliberately
//! do **not** come across, and pretending otherwise would be the worse answer:
//! per-slide `budget:` values, because speaking rate is not language
//! independent and a copied number is a wrong one; and the linter's overflow
//! verdict, because Japanese is denser per character and German longer per
//! word, so a translated deck has to be re-linted rather than assumed. Both are
//! reported by `slidx i18n apply` as work the author still owes.

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

mod apply;
mod catalogue;
mod extract;
mod protect;
mod segment;

pub use apply::{plan, Plan, Problem};
pub use catalogue::{Catalogue, Entry};
pub use protect::{mask, restore, Masked};
pub use segment::{Segment, SegmentKind};

use slidx_core::DeckParseOptions;

/// Every translatable segment of a deck, in source order.
pub fn extract(source: &str, options: &DeckParseOptions) -> Vec<Segment> {
    extract::segments(source, options)
}

/// A catalogue for `lang`, holding every segment of the deck.
///
/// `previous` carries a translator's work forward: an entry whose context and
/// source text both still match keeps its translation, and everything else
/// arrives empty. Without that, re-extracting after fixing a typo on slide one
/// would throw away the whole translation.
pub fn catalogue(
    source: &str,
    options: &DeckParseOptions,
    lang: &str,
    previous: Option<&Catalogue>,
) -> Catalogue {
    let mut fresh = Catalogue::of(lang, extract(source, options));

    if let Some(previous) = previous {
        fresh.carry_over(previous);
    }

    fresh
}

/// The source with a catalogue applied.
///
/// [`plan`] is the one a caller wants when it has to report anything, because
/// it carries what was skipped and why. This is the short form.
pub fn apply(source: &str, options: &DeckParseOptions, catalogue: &Catalogue) -> String {
    plan(source, options, catalogue).apply(source)
}
