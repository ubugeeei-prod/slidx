//! What the dialect check promises for input nobody wrote on purpose.
//!
//! The example-based contract lives beside each check and says what one mistake
//! turns into. This states the two properties that decide whether anybody leaves
//! the check switched on:
//!
//! 1. **It never panics.** It runs in the language server on every burst of
//!    typing, over a deck that is half written by construction.
//! 2. **It never reports a deck that works.** A checker that cries wolf gets
//!    switched off, and it takes the findings that mattered with it. This is
//!    stated the strong way: over generated decks built only from spellings the
//!    parser accepts, the check must say *nothing at all*.
//!
//! And two the findings themselves have to satisfy, for the same reason the
//! parser's diagnostics do: every one points at a slide that exists and a line
//! inside the file, or an editor's jump-to-error sends the author nowhere.
//!
//! Generated from a deterministic seed, so a failure is reproducible on a
//! machine that is not the one that found it. The vocabularies come from the
//! Rust that defines them rather than from a list here — a generator with its own
//! idea of the valid spellings would agree with a check that had the same wrong
//! idea.

use slidx_core::{parse_deck, AspectRatio, AutoSteps, Deck, DeckParseOptions};
use slidx_theme::{builtin, Transition};

fn parse(source: &str) -> Deck {
    parse_deck(source, &DeckParseOptions::default())
}

/// A deterministic pseudo-random generator.
///
/// xorshift64*, hand-rolled for the same reason the parser's properties do it:
/// no dependency, no clock, and the same seed on every machine.
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

/// Frontmatter lines that are all correct, drawn from the code that decides.
///
/// Every spelling here is one the parser reads and the check must therefore pass
/// in silence. Adding a theme or a transition upstream widens this set without
/// anyone editing the file, which is the point.
fn valid_frontmatter(rng: &mut Rng) -> String {
    let durations = ["1500", "25m", "25:00", "1h30m", "90s", "0s"];
    let themes: Vec<String> = builtin::all().into_iter().map(|theme| theme.id).collect();

    match rng.below(7) {
        0 => format!("title: {}\n", rng.pick(&["A talk", "日本語の発表", "🎤 Live"])),
        1 => format!("theme: {}\n", rng.pick(&themes)),
        2 => format!("aspect: \"{}\"\n", rng.pick(&AspectRatio::ALL).as_token()),
        3 => format!("duration: {}\n", rng.pick(&durations)),
        4 => format!("budget: {}\n", rng.pick(&durations)),
        5 => format!("transition: {}\n", rng.pick(&Transition::ALL).as_token()),
        _ => format!("autoSteps: {}\n", rng.pick(&AutoSteps::ALL).as_token()),
    }
}

/// Body fragments, including the marks a `steps:` entry may address.
const BODIES: [&str; 10] = [
    "# Heading\n",
    "## 日本語の見出し\n",
    "text\n",
    "- item\n",
    "The [result]{#result} was good.\n",
    "結果は [3.2倍速く]{#結果} なった。\n",
    "Latency [120ms]{#latency}[38ms]{#latency}.\n",
    "- one <!-- step -->\n",
    "```rust\nfn main() {}\n```\n",
    "\u{200b}\n",
];

/// Keys a generated `steps:` entry may target. Only ones the bodies declare.
const KEYS: [&str; 3] = ["result", "結果", "latency"];

/// One deck built entirely out of spellings the parser accepts.
///
/// The `steps:` entries are the delicate half: a target is emitted only
/// alongside the body that declares the key, because a step naming a mark on
/// another slide is a real mistake and this generator must not produce one.
fn generate_valid(rng: &mut Rng) -> String {
    let mut source = String::from("---\n");
    for _ in 0..1 + rng.below(4) {
        source.push_str(&valid_frontmatter(rng));
    }
    source.push_str("---\n\n");

    for slide in 0..1 + rng.below(4) {
        if slide > 0 {
            source.push_str("\n---\n\n");
        }

        let key = *rng.pick(&KEYS);
        let declaring = match key {
            "result" => "The [result]{#result} was good.\n",
            "結果" => "結果は [3.2倍速く]{#結果} なった。\n",
            _ => "Latency [120ms]{#latency}[38ms]{#latency}.\n",
        };

        if rng.below(3) == 0 {
            // A pipeline and the mark it addresses, in the same slide.
            source.push_str(&format!("steps:\n  - reveal: \"#{key}\"\n---\n\n"));
        }

        let body: &&str = rng.pick(&BODIES);
        source.push_str(body);
        source.push_str(declaring);
    }

    source
}

/// Fragments a deck actually passes through while somebody types one.
const HALF_TYPED: [&str; 17] = [
    "\n---\n",
    "---\ntitle: [unclosed\n---\n",
    "---\nsteps: not-a-list\n---\n",
    "---\nsteps:\n  - teleport: \".x\"\n---\n",
    "---\nduration:\n---\n",
    "---\nduration: fast\n---\n",
    "---\naspect: 16\n---\n",
    "---\ntheme: editoral\n---\n",
    "---\ntransition: cube\n---\n",
    "---\nbudget: {}\n---\n",
    "---\nsteps:\n  - reveal: \"#gone\"\n---\n",
    "# Heading\n",
    "[text]{#key\n",
    "<!-- notes:\nunterminated\n",
    "```\n",
    "# 日本語\n",
    "\u{200b}\n",
];

fn generate_half_typed(rng: &mut Rng) -> String {
    let count = 1 + rng.below(10);
    (0..count).map(|_| *rng.pick(&HALF_TYPED)).collect()
}

const CASES: usize = 3000;

#[test]
fn checking_half_typed_input_never_panics() {
    // The property that has to hold before the others are worth stating, and it
    // is asserted by arriving here. A panic inside the language server takes
    // every diagnostic away from the author, not just this one.
    let mut rng = Rng(0xD1A1_EC70_0000_0001);

    for _ in 0..CASES {
        let source = generate_half_typed(&mut rng);
        slidx_dialect::check(&parse(&source), &[], &slidx_dialect::Installed::default());
    }
}

#[test]
fn a_deck_written_only_in_spellings_the_parser_accepts_is_reported_on_for_nothing() {
    // The property that decides whether anybody leaves this switched on. One
    // false positive on a working deck and the whole group goes into `allow`.
    let mut rng = Rng(0xD1A1_EC70_0000_0002);

    for _ in 0..CASES {
        let source = generate_valid(&mut rng);
        let found =
            slidx_dialect::check(&parse(&source), &[], &slidx_dialect::Installed::default());

        assert!(found.is_empty(), "reported {found:?} for a valid deck:\n{source}");
    }
}

#[test]
fn every_finding_points_at_a_slide_that_exists() {
    // Findings are addressed to a slide, and an editor jumps to it. An index
    // past the end sends the author nowhere and teaches them to stop clicking.
    let mut rng = Rng(0xD1A1_EC70_0000_0003);

    for _ in 0..CASES {
        let source = generate_half_typed(&mut rng);
        let deck = parse(&source);
        let count = deck.slides.len() as u32;

        for finding in slidx_dialect::check(&deck, &[], &slidx_dialect::Installed::default()).iter()
        {
            let Some(index) = finding.span.slide_index else {
                panic!("{} names no slide, and every one of these is about a slide", finding.code);
            };

            assert!(index < count, "slide {index} of {count} for {} in {source:?}", finding.code);
        }
    }
}

#[test]
fn every_finding_points_at_the_line_the_slide_it_names_begins_on() {
    // Stated as an exact equality rather than as "inside the file", which is the
    // weaker property and not this crate's to hold: a finding's line *is*
    // `Slide::source_line`, and a slide declared by a frontmatter block that ends
    // the file reports one line past the last. That off-by-one belongs to the
    // parser and is recorded in `slidx_core/tests/parser_properties.rs` rather
    // than accommodated by a looser bound here.
    //
    // What this crate can promise is that it never invents a position: a finding
    // about slide four points at where slide four begins, so a fix is one jump
    // away even on a deck that is mid-edit.
    let mut rng = Rng(0xD1A1_EC70_0000_0004);

    for _ in 0..CASES {
        let source = generate_half_typed(&mut rng);
        let deck = parse(&source);

        for finding in slidx_dialect::check(&deck, &[], &slidx_dialect::Installed::default()).iter()
        {
            let index = finding.span.slide_index.expect("every finding names a slide");
            let slide = &deck.slides[index as usize];

            // A finding about the deck as a whole — its theme, its aspect ratio —
            // carries no line, because the key is the deck's rather than a place
            // in it.
            if finding.span.line == 0 {
                continue;
            }

            assert_eq!(
                finding.span.line, slide.source_line,
                "{} points away from slide {index} in {source:?}",
                finding.code
            );
        }
    }
}

#[test]
fn checking_the_same_deck_twice_gives_the_same_findings() {
    // No clock, no global state, no iteration order that depends on anything but
    // the deck. A CI job whose findings move between runs is one nobody trusts.
    let mut rng = Rng(0xD1A1_EC70_0000_0005);

    for _ in 0..CASES {
        let deck = parse(&generate_half_typed(&mut rng));

        assert_eq!(
            slidx_dialect::check(&deck, &[], &slidx_dialect::Installed::default()),
            slidx_dialect::check(&deck, &[], &slidx_dialect::Installed::default())
        );
    }
}

#[test]
fn the_generated_half_typed_decks_are_ones_the_check_finds_something_in() {
    // Every property above is satisfied by a check that reports nothing ever.
    let mut rng = Rng(0xD1A1_EC70_0000_0006);
    let mut reported = 0usize;

    for _ in 0..CASES {
        let deck = parse(&generate_half_typed(&mut rng));
        if !slidx_dialect::check(&deck, &[], &slidx_dialect::Installed::default()).is_empty() {
            reported += 1;
        }
    }

    assert!(reported > CASES / 4, "only {reported} of {CASES} half-typed decks reported anything");
}
