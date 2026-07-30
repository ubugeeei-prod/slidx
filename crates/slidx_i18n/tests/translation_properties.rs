//! What a translation promises about everything it did not come to change.
//!
//! The example-based contract lives beside each module and says what one
//! construct turns into. This states the other half — the part that is a
//! *promise to the author* — and it is stated as properties because the failure
//! mode is not a wrong output. It is a deck that renders perfectly, in the right
//! language, with the animation silently gone.
//!
//! The properties, in the order they would hurt:
//!
//! 1. **Nothing panics.** A catalogue is applied to a deck that is half typed by
//!    construction, and against a catalogue somebody hand-edited.
//! 2. **Every mark key survives.** `[120ms]{#latency}` is addressed by a
//!    `steps:` entry, and a key that moved is an animation that stops with
//!    nothing to say about why. This is the property the crate exists for.
//! 3. **Every slide id survives.** Ids are slugs of headings, so translating
//!    headings moves slides — including ones nobody translated, when two shared
//!    a title. Every deep link and every QR code into the deck addresses them.
//! 4. **A catalogue that translates nothing changes nothing**, byte for byte.
//! 5. **Fenced code is never touched.** A translated code comment no longer
//!    matches the recording of the talk.
//! 6. **Line endings survive**, so a deck written on Windows does not come back
//!    with a diff on every line.
//! 7. **Applying is idempotent**, so re-running after editing one string does
//!    not accumulate.
//!
//! Generated rather than enumerated, from a deterministic seed, for the same
//! reason `slidx_fmt/tests/format_properties.rs` is: a failure has to be
//! reproducible on a machine that is not the one that found it.

use std::collections::BTreeSet;

use slidx_core::{parse_deck, Deck, DeckParseOptions};
use slidx_i18n::{Catalogue, Entry};

fn options() -> DeckParseOptions {
    DeckParseOptions::default()
}

fn parse(source: &str) -> Deck {
    parse_deck(source, &options())
}

/// A deterministic pseudo-random generator.
///
/// xorshift64*, hand-rolled for the same reason the parser's properties do it:
/// no dependency, no clock, and the same seed produces the same cases on every
/// machine — which is the whole point of a property failure.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn pick<'a, T>(&mut self, options: &'a [T]) -> &'a T {
        &options[self.below(options.len())]
    }
}

/// Fragments a deck being translated is actually made of.
///
/// Weighted towards the things that must survive rather than towards prose: a
/// translator's mistakes are uninteresting next to a mark key that moved. The
/// CJK, emoji and zero-width fragments are here because a byte offset mistaken
/// for a character offset only shows up on input like this, and this crate is
/// nothing but byte offsets.
const FRAGMENTS: &[&str] = &[
    "# Heading\n",
    "##   Sub heading\n",
    "text\n",
    "\n",
    "\n---\n",
    "- item\n",
    "* starred item\n",
    "1. numbered item\n",
    "> a quotation\n",
    "a hand-wrapped sentence that\ncontinues on the next line\n",
    "- The venue Wi-Fi is down <!-- step -->\n",
    "Latency dropped to [120ms]{#latency}[38ms]{#latency}.\n",
    "The result was [3.2x faster]{#result .accent color=success}.\n",
    "See [the handbook](https://example.test/en/guide) for more.\n",
    "![A flame graph](./diagram.png)\n",
    "The `contrast/projector` rule catches it.\n",
    "Slides at https://example.test/talk after.\n",
    "<strong>Never</strong> on stage\n",
    "```rust\nlet frame = timeline.frame(step)?; // keep this\n```\n",
    "```md\n---\ntitle: quoted\n[x]{#k}\n```\n",
    "| Rule | Catches |\n| ---- | ------- |\n| `a`  | b       |\n",
    "<!-- notes:\nOpen with the outcome.\n-->\n",
    "<!-- notes: one line -->\n",
    "---\ntitle: A deck\ndescription: One sentence.\n---\n",
    "---\nlayout: statement\nbudget: 90s\n---\n",
    "---\nsteps:\n  - reveal: \"[data-slidx-mark=\\\"latency\\\"]\"\n---\n",
    "# Demo\n",
    "# 日本語の見出し\n",
    "結果は [3.2倍速く]{#結果} なった。\n",
    "絵文字 🎤 と組み合わせ文字 é\n",
    "50% faster, and %1 of the time\n",
    "\u{200b}\n",
    "\t\ttabbed\n",
    "# ",
];

fn generate(rng: &mut Rng) -> String {
    let count = 1 + rng.below(10);
    (0..count).map(|_| *rng.pick(FRAGMENTS)).collect()
}

/// Enough cases to walk the fragment interactions, and fast enough for CI.
const CASES: usize = 3000;

fn each_case(mut check: impl FnMut(&str)) {
    let mut rng = Rng(0x51D_1180_0000_0001);

    for _ in 0..CASES {
        let source = generate(&mut rng);
        check(&source);
    }
}

/// A catalogue translating every string of a deck.
///
/// The prose runs are wrapped in Japanese and the placeholders are left exactly
/// where they were, which is the shape of a careful translation. A careless one
/// is covered by the example tests: it is refused rather than applied.
fn translate_everything(source: &str) -> Catalogue {
    let entries = slidx_i18n::extract(source, &options())
        .into_iter()
        .map(|segment| Entry {
            target: japanese(&segment.text),
            context: segment.context,
            source: segment.text,
            ..Entry::default()
        })
        .collect();

    Catalogue { lang: "ja".to_string(), deck: "slides".to_string(), entries }
}

/// The text with Japanese around it and its interior untouched.
///
/// A careful translation, which is what these properties are about: every
/// placeholder is present, in order, and still against the bracket it belongs
/// to. A careless one is covered by the example tests — it is refused, named,
/// and never written.
fn japanese(text: &str) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }

    format!("これは{text}の訳です")
}

/// Every mark key a deck declares, and every selector its steps address.
///
/// Collected from the parsed model rather than from the source, because what
/// matters is what the runtime will look for and what will be there to find.
fn addresses(deck: &Deck) -> BTreeSet<String> {
    let mut found = BTreeSet::new();

    for slide in &deck.slides {
        for mark in &slide.marks {
            if let Some(key) = &mark.key {
                found.insert(format!("mark:{key}"));
            }
        }
        for action in &slide.steps.actions {
            for target in action.targets() {
                found.insert(format!("step:{target}"));
            }
        }
    }

    found
}

fn applied(source: &str) -> String {
    slidx_i18n::apply(source, &options(), &translate_everything(source))
}

#[test]
fn translating_half_typed_input_never_panics() {
    // The property that has to hold before any of the others are worth stating,
    // and it is asserted by arriving here. Decks are translated from a file
    // somebody is still editing.
    each_case(|source| {
        applied(source);
    });
}

#[test]
fn every_mark_key_and_every_step_target_survives_a_full_translation() {
    // The failure this whole crate exists to prevent, and the only one that is
    // completely silent: a `steps:` entry addressing a key that was translated
    // leaves a deck that builds, renders, and does not animate.
    each_case(|source| {
        let before = addresses(&parse(source));
        let after = addresses(&parse(&applied(source)));

        assert_eq!(before, after, "an address moved in {source:?}");
    });
}

#[test]
fn every_slide_id_survives_a_full_translation_or_the_author_is_told_it_did_not() {
    // Ids are slugs of headings, so every heading translated is a slide that
    // wanted to move. Each one that would have is pinned instead, which is what
    // keeps a deep link and a printed QR code pointing at the same slide.
    //
    // The second half of the name is load-bearing rather than a hedge. A deck
    // with no frontmatter whose body opens on a separator cannot be given a
    // block without moving where the parser thinks its first slide starts — so
    // the pin is withheld and reported, never written and hoped for. A deck read
    // through `slidx i18n` cannot be in that shape, because joining trims each
    // file, but the crate does not get to assume its caller.
    each_case(|source| {
        let ids = |deck: &Deck| -> Vec<String> {
            deck.slides.iter().map(|slide| slide.id.clone()).collect()
        };
        let plan = slidx_i18n::plan(source, &options(), &translate_everything(source));

        if plan.problems.contains(&slidx_i18n::Problem::CouldNotPin) {
            return;
        }

        assert_eq!(
            ids(&parse(source)),
            ids(&parse(&plan.apply(source))),
            "a slide id moved in {source:?}"
        );
    });
}

#[test]
fn a_translation_never_changes_how_many_slides_or_stops_a_deck_has() {
    // A deck that gained a slide or lost a stop would be a different talk, and
    // the person giving it has rehearsed the other one.
    each_case(|source| {
        let before = parse(source);
        let after = parse(&applied(source));

        assert_eq!(before.slides.len(), after.slides.len(), "slide count for {source:?}");

        for (one, two) in before.slides.iter().zip(&after.slides) {
            assert_eq!(one.timeline.len(), two.timeline.len(), "stops for {source:?}");
            assert_eq!(one.marks.len(), two.marks.len(), "marks for {source:?}");
            assert_eq!(one.notes.len(), two.notes.len(), "notes for {source:?}");
        }
    });
}

#[test]
fn a_catalogue_that_translates_nothing_returns_the_source_byte_for_byte() {
    // Not "produces the same text" — plans no splice at all, so the bytes are
    // never read and cannot change. It is what makes a half-finished
    // translation safe to apply, which is the only state a translation is in
    // until the day it is finished.
    each_case(|source| {
        let plan = slidx_i18n::plan(source, &options(), &Catalogue::default());

        assert!(plan.is_empty(), "an empty catalogue planned an edit for {source:?}");
        assert_eq!(plan.apply(source), source);
    });
}

#[test]
fn a_deck_quoted_inside_a_fenced_code_block_is_left_completely_alone() {
    // Including its comments: a translated code comment no longer matches the
    // recording of the talk, and a talk about slidx puts this dialect on a slide
    // on purpose.
    //
    // Four backticks, so nothing the generator produces can close the fence
    // early — the property is about content that really is inside one.
    each_case(|source| {
        let quoted = format!("# Quoting a deck\n\n````md\n{source}\n````\n");

        assert!(applied(&quoted).contains(source), "a fence was translated: {source:?}");
    });
}

#[test]
fn a_deck_written_with_crlf_comes_back_with_crlf() {
    // Windows checkouts are in CI, and converting line endings is a diff on
    // every line of a file nobody edited.
    each_case(|source| {
        let windows = source.replace('\n', "\r\n");
        let translated = applied(&windows);

        for (index, _) in translated.match_indices('\n') {
            assert!(
                index > 0 && translated.as_bytes()[index - 1] == b'\r',
                "a lone newline at {index} in {translated:?}"
            );
        }
    });
}

#[test]
fn applying_the_same_catalogue_twice_changes_nothing_the_second_time() {
    // A translation is applied again every time one string of it is fixed. A
    // pass that accumulated would double a pinned id or re-wrap a sentence.
    each_case(|source| {
        let catalogue = translate_everything(source);
        let once = slidx_i18n::apply(source, &options(), &catalogue);
        let twice = slidx_i18n::apply(&once, &options(), &catalogue);

        assert_eq!(once, twice, "a second pass rewrote {source:?}");
    });
}

#[test]
fn a_catalogue_survives_the_file_it_is_stored_in() {
    // The catalogue leaves the process and comes back through a translator's
    // tool. A field that did not round-trip would be a translation lost.
    each_case(|source| {
        let catalogue = translate_everything(source);

        assert_eq!(Catalogue::from_po(&catalogue.to_po()), catalogue, "for {source:?}");
    });
}

#[test]
fn the_generated_cases_are_ones_that_actually_get_translated() {
    // Every property above is satisfied by a pass that does nothing at all, so
    // the suite needs to know these decks are not all prose-free. Without this
    // the whole file could go quietly green on an extractor that stopped
    // finding anything.
    let mut changed = 0usize;
    let mut with_marks = 0usize;

    each_case(|source| {
        if applied(source) != source {
            changed += 1;
        }
        if !addresses(&parse(source)).is_empty() {
            with_marks += 1;
        }
    });

    assert!(changed > CASES / 2, "only {changed} of {CASES} generated decks were translated");
    assert!(with_marks > CASES / 4, "only {with_marks} of {CASES} had an address to preserve");
}
