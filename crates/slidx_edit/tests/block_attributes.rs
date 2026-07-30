//! What an attribute line survives.
//!
//! A block attribute is the smallest thing in a deck that only an editor ever
//! writes. `{.side}` on a line of its own is not prose an author reaches for
//! naturally, so if an unrelated edit moves it, reflows it, or drops it, nothing
//! about the slide looks wrong on the canvas — the block simply stops being in
//! the region the author put it in, and the diff blames the wrong line.
//!
//! So the property is stated at the byte level rather than at the model level:
//!
//! 1. **Everything outside the spliced range is the same bytes.** Not "parses
//!    the same", not "looks the same". The same bytes, in the same order.
//! 2. **A block the operation did not name keeps its attributes.** Which is the
//!    model-level consequence, and the one an author would notice.
//! 3. **An attribute line is never what an operation reaches for.** Retitling a
//!    slide, writing its notes, and setting a frontmatter key all splice ranges
//!    that do not overlap one.
//!
//! Generated rather than enumerated, from a deterministic seed, for the reason
//! `crates/slidx_core/tests/parser_properties.rs` gives: a property failure has
//! to be reproducible on a machine that is not the one that found it.

use slidx_core::{find_blocks, parse_deck, ByteSpan, DeckParseOptions, StepAction};
use slidx_edit::{plan, slide_spans, Edit, EditOp};

fn parse(source: &str) -> slidx_core::Deck {
    parse_deck(source, &DeckParseOptions::default())
}

/// A deterministic pseudo-random generator: xorshift64*, hand-rolled so a
/// failure reproduces from the seed alone.
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

/// Fragments a deck with placed blocks is made of.
///
/// The awkward entries are the point: an attribute line whose group is quoted,
/// one that is a whole chunk of its own, one inside a fence where it is code
/// rather than an attribute, and a paragraph that merely opens with a brace.
const FRAGMENTS: &[&str] = &[
    "\n---\n",
    "---\nlayout: split\n---\n",
    "---\nlayout: aside\nbudget: 90s\n---\n",
    "# Heading\n",
    "## Sub heading\n",
    "Some prose.\n",
    "\n",
    "{.side}\n![A diagram](./a.svg)\n",
    "{.right}\n- one\n- two\n",
    "{#hero .main title=\"A long title\"}\n# Placed\n",
    "{.side}\n\n# Separated by a blank line\n",
    "{.side}\n",
    "{not an attribute group\n",
    "{}\nProse under an empty group.\n",
    "```md\n{.side}\n```\n",
    "- item <!-- step -->\n",
    "<!-- notes:\nsaid out loud\n-->\n",
    "A [word]{#w .accent} inside a line.\n",
    "# 日本語の見出し\n",
    "{.side}\n# 日本語の見出し\n",
];

fn generate(rng: &mut Rng) -> String {
    let count = 2 + rng.below(10);
    (0..count).map(|_| *rng.pick(FRAGMENTS)).collect()
}

/// Enough cases to cross the fragments with each other, and fast enough for CI.
const CASES: usize = 1500;

fn each_case(mut check: impl FnMut(&str)) {
    let mut rng = Rng(0x5EED_B10C_C0DE_0001);

    for _ in 0..CASES {
        let source = generate(&mut rng);
        check(&source);
    }
}

/// Operations that name something other than a block.
fn unrelated(source: &str) -> Vec<EditOp> {
    let last = parse(source).slides.len() - 1;

    vec![
        EditOp::SetHeading { slide: 0.into(), text: "Retitled".into() },
        EditOp::SetHeading { slide: last.into(), text: "Retitled".into() },
        EditOp::SetNotes { slide: 0.into(), notes: "said out loud".into() },
        EditOp::SetField { slide: 0.into(), key: "theme".into(), value: "terminal".into() },
        EditOp::SetField { slide: last.into(), key: "budget".into(), value: "45s".into() },
        EditOp::AddStep { slide: 0.into(), action: StepAction::reveal(".added") },
        EditOp::InsertSlide { at: 0, body: "# Inserted".into() },
        EditOp::InsertSlide { at: last + 1, body: "# Inserted".into() },
    ]
}

/// Every attribute line in a source, with the bytes it occupies.
fn attribute_lines(source: &str) -> Vec<(ByteSpan, String)> {
    slide_spans(source, &DeckParseOptions::default())
        .iter()
        .flat_map(|slide| {
            let body = slide.body;
            find_blocks(body.slice(source)).into_iter().filter_map(move |found| {
                let line = found.attribute_line?;
                Some((line.shifted(body.start), line.slice(body.slice(source)).to_string()))
            })
        })
        .collect()
}

/// The runs of bytes an edit leaves between its splices, on both sides.
///
/// Each splice can change the length of what it replaced, so the second side is
/// walked with the running shift rather than with the original offsets.
fn between(source: &str, after: &str, edit: &Edit) -> (Vec<String>, Vec<String>) {
    let mut before_runs = Vec::new();
    let mut after_runs = Vec::new();
    let mut before_at = 0usize;
    let mut after_at = 0usize;

    for splice in edit.splices() {
        let width = splice.span.start - before_at;

        before_runs.push(source[before_at..splice.span.start].to_string());
        after_runs.push(after[after_at..after_at + width].to_string());

        before_at = splice.span.end;
        after_at += width + splice.text.len();
    }

    before_runs.push(source[before_at..].to_string());
    after_runs.push(after[after_at..].to_string());

    (before_runs, after_runs)
}

#[test]
fn every_byte_outside_a_spliced_range_is_the_same_byte_afterwards() {
    // The claim the whole crate rests on, checked here on decks that contain
    // attribute lines. `apply` is what produces the second string, so this is
    // an assertion about the *ranges* an operation chooses: one that reached
    // wider than it needed to would show up as different bytes outside it.
    each_case(|source| {
        for op in unrelated(source) {
            let edit = plan(source, &DeckParseOptions::default(), &op).unwrap();
            let after = edit.apply(source);
            let (before_runs, after_runs) = between(source, &after, &edit);

            assert_eq!(
                before_runs, after_runs,
                "{op:?} changed bytes it did not name in {source:?}"
            );
        }
    });
}

#[test]
fn an_unrelated_edit_never_splices_an_attribute_line() {
    // Retitling a slide is a heading. Writing its notes is a comment. Neither is
    // ever the line that says where a block goes, so neither may overlap one.
    each_case(|source| {
        let lines = attribute_lines(source);
        if lines.is_empty() {
            return;
        }

        for op in unrelated(source) {
            let edit = plan(source, &DeckParseOptions::default(), &op).unwrap();

            for splice in edit.splices() {
                for (line, text) in &lines {
                    let overlaps = splice.span.start < line.end && line.start < splice.span.end;
                    assert!(
                        !overlaps,
                        "{op:?} spliced {:?} across the attribute line {text:?} in {source:?}",
                        splice.span
                    );
                }
            }
        }
    });
}

#[test]
fn a_block_the_operation_did_not_name_still_carries_its_attributes() {
    // The model-level consequence of the byte-level property, and the one an
    // author would actually notice: a block that quietly lost `.side` is a block
    // that quietly left its region.
    each_case(|source| {
        let before: Vec<Vec<slidx_core::Attributes>> = parse(source)
            .slides
            .iter()
            .map(|slide| slide.blocks.iter().map(|block| block.attributes.clone()).collect())
            .collect();

        for op in [
            EditOp::SetNotes { slide: 0.into(), notes: "said out loud".into() },
            EditOp::SetField { slide: 0.into(), key: "theme".into(), value: "terminal".into() },
        ] {
            let after = plan(source, &DeckParseOptions::default(), &op).unwrap().apply(source);
            let deck = parse(&after);

            // Writing the deck's own frontmatter into a file that had none can
            // change where the first slide starts, and that is the slide the
            // operation named. Every other slide is what this is about.
            if deck.slides.len() != before.len() {
                continue;
            }

            for (index, expected) in before.iter().enumerate().skip(1) {
                let found: Vec<slidx_core::Attributes> = deck.slides[index]
                    .blocks
                    .iter()
                    .map(|block| block.attributes.clone())
                    .collect();

                assert_eq!(&found, expected, "{op:?} disturbed slide {index} of {source:?}");
            }
        }
    });
}

#[test]
fn an_attribute_line_is_never_rendered_onto_the_slide() {
    // The failure this prevents is `{.side}` appearing as a paragraph on a
    // projector, which is what an author sees if the line is recognised
    // everywhere except in the content the shell is handed.
    each_case(|source| {
        for slide in &parse(source).slides {
            let mut fences = slidx_core::scanner::FenceTracker::new();

            for line in slide.content.lines() {
                // A line inside a fence is code, and code that looks like an
                // attribute group is a slide doing its job.
                if !fences.feed(line) {
                    continue;
                }

                assert!(
                    !is_attribute_line(line),
                    "the attribute line {line:?} reached the slide body of {source:?}"
                );
            }
        }
    });
}

/// True when a line is nothing but an attribute group.
fn is_attribute_line(line: &str) -> bool {
    line.trim()
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .and_then(slidx_core::attributes::parse)
        .is_some()
}
