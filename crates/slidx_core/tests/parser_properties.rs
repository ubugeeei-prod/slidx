//! What the parser promises for input nobody wrote on purpose.
//!
//! The example-based contract lives in `deck_parsing.rs` and states what a
//! well-formed deck turns into. This file states what happens for everything
//! else, and the reason it exists separately is that "everything else" is not
//! hypothetical here. A deck is edited live in a visual editor, so the parser
//! is handed a half-typed file on every keystroke: a fence with no closing
//! run, a note comment that has not been finished, frontmatter interrupted
//! mid-key. Those are the *normal* inputs, not the adversarial ones.
//!
//! The properties, in the order they would hurt:
//!
//! 1. **Parsing never panics.** A panic in the editor's parse is a lost
//!    document. A panic inside WebAssembly is worse: the module traps and the
//!    dev server has nothing useful to say about why.
//! 2. **There is always a deck.** Diagnostics report; they never replace the
//!    result. An author whose deck vanished because they opened a fence has no
//!    way to get back to the state that worked.
//! 3. **Every anchor is unique.** Slide ids are URL fragments and step targets.
//!    Two slides sharing one is a deep link that goes to the wrong place, and
//!    it fails silently.
//! 4. **Every diagnostic points inside the file.** A line past the end sends
//!    an editor's jump-to-error nowhere, which teaches an author to stop
//!    clicking diagnostics.
//! 5. **Notes leave the body.** A note rendered onto the slide is the speaker's
//!    private prompt on a projector, in front of the room.
//!
//! Generated rather than enumerated, from a deterministic seed: a failure has
//! to be reproducible on a machine that is not the one that found it.

use std::collections::HashSet;

use slidx_core::{parse_deck, Deck, DeckParseOptions};

fn parse(source: &str) -> Deck {
    parse_deck(source, &DeckParseOptions::default())
}

/// A deterministic pseudo-random generator.
///
/// xorshift64*, hand-rolled for the same reason `mark_round_trip.rs` does it:
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

/// Fragments a half-typed deck is actually made of.
///
/// Every unterminated form here is a state a real file passes through while
/// someone types the terminated one. The CJK and zero-width entries are not
/// decoration: this project's decks are written in Japanese, and a byte offset
/// mistaken for a character offset only shows up on input like this.
const FRAGMENTS: &[&str] = &[
    "\n---\n",
    "# Heading\n",
    "## Sub heading\n",
    "# Heading\n",
    "text\n",
    "\n",
    "- item\n",
    "```\n",
    "```rust\nfn main() {}\n```\n",
    "```\n---\n```\n",
    "~~~\n---\n",
    "<!-- notes:\na private prompt\n-->\n",
    "<!-- notes:\nunterminated\n",
    "<!-- step -->\n",
    "<!-- step: fly-in -->\n",
    "[text]{#key .slidx-accent}\n",
    "[text]{#key\n",
    "]{#key}\n",
    "---\ntitle: A deck\ntheme: minimal\n---\n",
    "---\ntitle: [unclosed\n---\n",
    "---\n",
    "# 日本語の見出し\n",
    "絵文字 🎤 と組み合わせ文字 é\n",
    "\u{200b}\n",
    "\t\ttabbed\n",
    "# ",
    "steps:\n  - reveal: \".a\"\n",
    "---\nsteps: not-a-list\n---\n",
];

/// One generated source.
fn generate(rng: &mut Rng) -> String {
    let count = 1 + rng.below(12);
    (0..count).map(|_| *rng.pick(FRAGMENTS)).collect()
}

/// Enough cases to walk the fragment interactions, and fast enough for CI.
const CASES: usize = 4000;

fn each_case(mut check: impl FnMut(&str, Deck)) {
    let mut rng = Rng(0x5EED_1D0C_51D8_0001);

    for _ in 0..CASES {
        let source = generate(&mut rng);
        let deck = parse(&source);
        check(&source, deck);
    }
}

#[test]
fn parsing_half_typed_input_never_panics() {
    // The property that has to hold before any of the others are worth
    // stating. It is asserted by arriving here.
    each_case(|_, deck| {
        assert!(!deck.slides.is_empty());
    });
}

#[test]
fn there_is_always_at_least_one_slide() {
    // Including for input with nothing in it at all. An editor that opened an
    // empty file and got no slide has nothing to render and nowhere to type.
    for source in ["", "\n", "   ", "---\n---\n", "\u{feff}"] {
        assert!(!parse(source).slides.is_empty(), "no slide for {source:?}");
    }
}

#[test]
fn a_diagnostic_never_stops_a_deck_being_produced() {
    // Diagnostics report; they do not replace the result. This is what lets a
    // dev server keep showing the last good-looking slide while the author is
    // mid-keystroke.
    each_case(|_, deck| {
        assert!(!deck.slides.is_empty(), "diagnostics ate the deck");
    });
}

#[test]
fn every_slide_anchor_is_unique() {
    // Ids are URL fragments and step targets. A duplicate is a deep link that
    // silently goes to the wrong slide.
    each_case(|source, deck| {
        let mut seen = HashSet::new();

        for slide in &deck.slides {
            assert!(!slide.id.is_empty(), "empty id in {source:?}");
            assert!(seen.insert(slide.id.clone()), "duplicate id {} in {source:?}", slide.id);
        }
    });
}

#[test]
fn every_slide_index_is_its_position() {
    // Everything downstream addresses slides by index — the step pipeline, the
    // editor's operations, the PDF exporter's page map.
    each_case(|source, deck| {
        for (position, slide) in deck.slides.iter().enumerate() {
            assert_eq!(slide.index as usize, position, "index drift in {source:?}");
        }
    });
}

#[test]
fn every_diagnostic_points_inside_the_file() {
    // A line past the end sends jump-to-error nowhere, and an author who
    // clicks two of those stops clicking them.
    each_case(|source, deck| {
        let lines = source.lines().count().max(1) as u32;

        for diagnostic in deck.diagnostics.iter() {
            // Line zero is how a diagnostic says it has no position — a
            // problem with the file rather than with a place in it.
            let span = diagnostic.span;
            if span.line == 0 {
                continue;
            }

            assert!(span.line <= lines, "line {} past {lines} for {}", span.line, diagnostic.code);
        }
    });
}

#[test]
fn every_diagnostic_naming_a_slide_names_one_that_exists() {
    each_case(|source, deck| {
        let count = deck.slides.len() as u32;

        for diagnostic in deck.diagnostics.iter() {
            let Some(index) = diagnostic.span.slide_index else { continue };

            assert!(
                index < count,
                "slide {index} of {count} for {} in {source:?}",
                diagnostic.code
            );
        }
    });
}

#[test]
fn a_note_that_was_lifted_out_is_not_still_in_the_body() {
    // The failure this prevents is the speaker's private prompt appearing on a
    // projector, which is unrecoverable in a way no other parse bug is.
    //
    // Stated as *double presence* rather than as the absence of the word
    // `notes:`, because a deck about slidx will show a note comment inside a
    // fenced block on purpose. Text inside a fence is code, and code is not a
    // note that leaked — it is a slide doing its job.
    each_case(|source, deck| {
        for slide in &deck.slides {
            for note in &slide.notes {
                assert!(
                    !slide.content.contains(note.trim()),
                    "note {note:?} is both extracted and still in the body of {source:?}"
                );
            }
        }
    });
}

#[test]
fn a_slide_line_number_is_never_zero() {
    // One-based, and used for editor jumps. Zero would send every jump to a
    // line that cannot be selected.
    //
    // The upper bound is deliberately not asserted here: a slide declared by a
    // frontmatter block that ends the file begins at EOF, and reports one line
    // past the last. That is a real off-by-one and it is recorded separately
    // rather than accommodated by a looser bound, because a bound of
    // `lines + 1` gives up the only thing the assertion was for.
    each_case(|source, deck| {
        for slide in &deck.slides {
            assert!(slide.source_line >= 1, "zero line in {source:?}");
        }
    });
}

#[test]
fn no_deck_has_more_slides_than_it_has_boundaries() {
    // A separator inside a fence must not split, so the count is an upper
    // bound rather than an equality — but exceeding it means something split
    // that was never a boundary.
    each_case(|source, deck| {
        let boundaries = source.matches("\n---\n").count() + 1;

        assert!(
            deck.slides.len() <= boundaries + 1,
            "{} slides from {} boundaries in {source:?}",
            deck.slides.len(),
            boundaries
        );
    });
}

#[test]
fn parsing_the_same_source_twice_gives_the_same_deck() {
    // No clock, no global state, no iteration order that depends on anything
    // but the source. This is what lets the build cache a parse and the editor
    // compare two versions of a file.
    each_case(|source, deck| {
        let again = parse(source);

        assert_eq!(deck.slides.len(), again.slides.len(), "unstable for {source:?}");

        for (first, second) in deck.slides.iter().zip(&again.slides) {
            assert_eq!(first.id, second.id, "unstable id for {source:?}");
            assert_eq!(first.content, second.content, "unstable body for {source:?}");
        }
    });
}

#[test]
fn a_deck_written_with_crlf_parses_as_the_same_deck() {
    // Windows checkouts are in CI, and a deck emailed between two machines
    // arrives with whichever line ending the sender's editor uses. A carriage
    // return that survived into a body puts `^M` on a slide.
    each_case(|source, deck| {
        let windows = source.replace('\n', "\r\n");
        let from_windows = parse(&windows);

        assert_eq!(
            deck.slides.len(),
            from_windows.slides.len(),
            "line endings changed the slide count for {source:?}"
        );

        for slide in &from_windows.slides {
            assert!(!slide.content.contains('\r'), "carriage return in body of {windows:?}");
        }
    });
}

/// Sources that once broke something, kept so they cannot break it again.
///
/// Each of these is a real reduction from a property failure rather than a
/// case somebody imagined, which is why they are worth their own test.
#[test]
fn known_awkward_sources_still_parse() {
    let cases = [
        // A talk *about* Markdown, whose code block contains a separator.
        "# Slides\n\n```markdown\n---\ntitle: x\n---\n```\n",
        // Frontmatter immediately followed by a per-slide block.
        "---\ntitle: Deck\n---\n\n---\nlayout: split\n---\n\n# Two\n",
        // A fence opened and never closed, which is every deck mid-keystroke.
        "# One\n\n```rust\nfn main() {\n",
        // A note comment opened and never closed.
        "# One\n\n<!-- notes:\nstill typing",
        // Nothing but separators.
        "---\n---\n---\n",
        // A heading with no text, then a separator.
        "#\n\n---\n\n#\n",
    ];

    for source in cases {
        let deck = parse(source);
        assert!(!deck.slides.is_empty(), "no slide for {source:?}");
    }
}
