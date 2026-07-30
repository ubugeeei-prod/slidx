//! What the formatter promises about the file it did not come to change.
//!
//! The example-based contract lives beside each rule and says what a construct
//! turns into. This file states the other half — the part that is a *promise to
//! the author* rather than a feature — and it is stated as properties because
//! the failure mode is not a wrong output. It is a diff on a line nobody
//! touched, in a file somebody is about to review, discovered after the commit.
//!
//! The properties, in the order they would hurt:
//!
//! 1. **Formatting never panics.** A deck is formatted from an editor, on a
//!    file that is half typed by construction.
//! 2. **The deck means the same thing.** Slides, ids, bodies, notes and
//!    frontmatter all survive. A formatter that changed what a slide says would
//!    be worse than no formatter, because nobody re-reads a deck after running
//!    one.
//! 3. **Prose is not touched.** Every line slidx does not own comes out byte
//!    for byte, including the ones with CJK, emoji, zero-width spaces and tabs
//!    in them.
//! 4. **A fenced code block is not touched.** A talk about slidx puts this
//!    dialect on a slide on purpose.
//! 5. **Formatting is idempotent.** A second run is a no-op, so a repository
//!    that formats in CI does not oscillate.
//! 6. **Line endings survive.** A deck written on Windows does not come back
//!    with a diff on every line.
//!
//! Generated rather than enumerated, from a deterministic seed, for the same
//! reason `slidx_core/tests/parser_properties.rs` is: a failure has to be
//! reproducible on a machine that is not the one that found it.

use slidx_core::{parse_deck, Deck, DeckParseOptions};

fn options() -> DeckParseOptions {
    DeckParseOptions::default()
}

fn format(source: &str) -> String {
    slidx_fmt::format(source, &options())
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

/// Fragments a deck the formatter will be run on is actually made of.
///
/// Half of these are deliberately non-canonical, because a formatter with
/// nothing to do proves nothing. The other half are prose that must survive: a
/// formatter that is byte-exact on ASCII and wrong on Japanese is no use to
/// anyone who writes decks in it, and a byte offset mistaken for a character
/// offset only shows up on input like this.
const FRAGMENTS: &[&str] = &[
    "\n---\n",
    "\n  ---  \n",
    "\n----\n",
    "\n- - -\n",
    "# Heading\n",
    "##   Sub heading\n",
    "text\n",
    "\n",
    "- item\n",
    "* starred item\n",
    "a hand-wrapped sentence that\ncontinues on the next line\n",
    "```\n",
    "```rust\nfn main() {}\n```\n",
    "```md\n---\ntheme: minimal\n[x]{.a #k}\n<!--step-->\n<!--note: x-->\n```\n",
    "~~~\n---\n",
    "<!-- notes:\na private prompt\n-->\n",
    "<!--note:  unspaced  -->\n",
    "<!-- notes: one line -->\n",
    "<!-- notes:\nunterminated\n",
    "<!-- step -->\n",
    "<!--step-->\n",
    "<!--step: fly-in-->\n",
    "<!-- step  :  sparkle -->\n",
    "<!-- stepper -->\n",
    "[text]{#key .slidx-accent}\n",
    "[text]{color=danger .a #k}\n",
    "[text]{accent}\n",
    "[a [nested] b]{.a #k}\n",
    "[text]{#key\n",
    "]{#key}\n",
    "see [the docs](https://example.com)\n",
    "---\ntitle: A deck\ntheme: minimal\n---\n",
    "---\ntheme: minimal\ntitle: A deck\ngridOverlay: true\n---\n",
    "---\nsteps:\n     - reveal: \".a\"\n---\n",
    "---\ndescription: |\n    literal\n      value\n---\n",
    "---\ntitle: [unclosed\n---\n",
    "---\n",
    "# 日本語の見出し\n",
    "結果は [3.2倍速く]{color=danger #結果} なった。\n",
    "絵文字 🎤 と組み合わせ文字 é\n",
    "\u{200b}\n",
    "\t\ttabbed\n",
    "# ",
    "| a | b |\n| - | - |\n| 1 | 2 |\n",
];

fn generate(rng: &mut Rng) -> String {
    let count = 1 + rng.below(12);
    (0..count).map(|_| *rng.pick(FRAGMENTS)).collect()
}

/// Enough cases to walk the fragment interactions, and fast enough for CI.
const CASES: usize = 4000;

fn each_case(mut check: impl FnMut(&str)) {
    let mut rng = Rng(0xF01A_7ED0_0000_0001);

    for _ in 0..CASES {
        let source = generate(&mut rng);
        check(&source);
    }
}

#[test]
fn formatting_half_typed_input_never_panics() {
    // The property that has to hold before any of the others are worth
    // stating, and it is asserted by arriving here. A panic inside the language
    // server takes formatting away from the author with no explanation.
    each_case(|source| {
        format(source);
    });
}

#[test]
fn formatting_is_idempotent() {
    // Stated over generated input rather than examples because the interesting
    // failures are compositional: a rule whose output the next rule rewrites
    // again oscillates, and a repository that formats in CI never goes green.
    each_case(|source| {
        let once = format(source);
        let twice = format(&once);

        assert_eq!(once, twice, "a second run rewrote {source:?}");
    });
}

#[test]
fn the_deck_a_formatted_file_parses_to_is_the_deck_it_parsed_to_before() {
    // The formatter reorders somebody's YAML and rewrites their comments.
    // Nothing about what the audience sees may move.
    each_case(|source| {
        let before = parse(source);
        let after = parse(&format(source));

        assert_eq!(before.slides.len(), after.slides.len(), "slide count for {source:?}");
        assert_eq!(before.meta.raw, after.meta.raw, "frontmatter for {source:?}");

        for (one, two) in before.slides.iter().zip(&after.slides) {
            assert_eq!(one.id, two.id, "slide id for {source:?}");
            assert_eq!(one.content, two.content, "slide body for {source:?}");
            assert_eq!(one.notes, two.notes, "notes for {source:?}");
            assert_eq!(one.marks, two.marks, "marks for {source:?}");
            assert_eq!(one.timeline.len(), two.timeline.len(), "stops for {source:?}");
            assert_eq!(one.frontmatter, two.frontmatter, "slide frontmatter for {source:?}");
        }
    });
}

/// True when slidx owns nothing on this line, so it must come out unchanged.
///
/// Computed here rather than asked of the formatter, which is the point: the
/// property is only worth stating if the two disagree about what a construct is.
/// Deliberately over-broad — a line holding a `:` might be frontmatter and a
/// line holding a brace might be a mark — because the assertion is about the
/// lines that are *certainly* prose, and being certain is what makes a failure
/// mean something.
fn is_prose(line: &str) -> bool {
    !line.contains(['{', '}', ':', '[', ']'])
        && !line.contains("<!--")
        && !line.contains("-->")
        && line.trim() != "---"
}

#[test]
fn every_line_slidx_does_not_own_comes_out_byte_for_byte() {
    // Prose, bullet markers, hand wrapping, tabs, CJK, emoji, a zero-width
    // space. This is the promise: the formatter is not a Markdown formatter and
    // has no opinion about any of it.
    each_case(|source| {
        let before: Vec<&str> = source.lines().filter(|line| is_prose(line)).collect();
        let formatted = format(source);
        let after: Vec<&str> = formatted.lines().filter(|line| is_prose(line)).collect();

        assert_eq!(before, after, "prose changed in {source:?}");
    });
}

#[test]
fn a_deck_quoted_inside_a_fenced_code_block_is_left_completely_alone() {
    // A deck about slidx shows this dialect on a slide, and a formatter that
    // rewrote the sample would make the slide wrong about the thing it teaches.
    //
    // Four backticks, so nothing the generator produces can close the fence
    // early — the property is about content that really is inside one.
    each_case(|source| {
        let quoted = format!("# Quoting a deck\n\n````md\n{source}\n````\n");

        assert_eq!(format(&quoted), quoted, "a fence was formatted: {source:?}");
    });
}

#[test]
fn a_deck_written_with_crlf_comes_back_with_crlf() {
    // Windows checkouts are in CI, and a deck emailed between two machines
    // arrives with whichever line ending the sender's editor uses. Converting
    // them is a diff on every line of a file nobody edited.
    each_case(|source| {
        let windows = source.replace('\n', "\r\n");
        let formatted = format(&windows);

        for (index, _) in formatted.match_indices('\n') {
            assert!(
                index > 0 && formatted.as_bytes()[index - 1] == b'\r',
                "a lone newline at {index} in {formatted:?}"
            );
        }
    });
}

#[test]
fn the_generated_cases_are_ones_the_formatter_actually_changes() {
    // Every property above is satisfied by a formatter that does nothing at
    // all, so the suite needs to know that these decks are not all already
    // canonical. Without this the whole file could go quietly green on a rule
    // that stopped firing.
    let mut changed = 0usize;
    each_case(|source| {
        if !slidx_fmt::plan(source, &options()).is_empty() {
            changed += 1;
        }
    });

    assert!(changed > CASES / 4, "only {changed} of {CASES} generated decks were formatted");
}

#[test]
fn formatting_the_same_source_twice_gives_the_same_bytes() {
    // No clock, no global state, no iteration order that depends on anything
    // but the source. A formatter that was not a pure function of the file
    // would show up as a repository that will not stay formatted.
    each_case(|source| {
        assert_eq!(format(source), format(source), "unstable for {source:?}");
    });
}

/// Sources that once broke something, kept so they cannot break it again.
#[test]
fn known_awkward_sources_are_formatted_without_surprises() {
    let cases = [
        // A `|` value at the end of a block has no newline after it and one
        // that is not, does — so reordering it would change the value.
        "---\ndescription: |\n  literal\ntitle: T\n---\n\n# One\n",
        // Frontmatter that is not a mapping. The parser reports it; the
        // formatter must not try to tidy it.
        "---\n- a\n- b\n---\n\n# One\n",
        // A separator whose spelling is already canonical, inside a deck whose
        // frontmatter quotes one.
        "---\ntitle: \"a --- dash\"\n---\n\n# One\n\n---\n\n# Two\n",
        // A mark whose text contains the characters the serialiser escapes.
        "[a [nested] b]{.a #k}\n",
        // A note comment that is never closed, which every note passes through.
        "# One\n\n<!-- notes:\nstill typing",
    ];

    for source in cases {
        let once = format(source);
        assert_eq!(format(&once), once, "not idempotent: {source:?}");
        assert_eq!(parse(source).slides.len(), parse(&once).slides.len(), "{source:?}");
    }
}
