//! Where the time goes when a deck is built.
//!
//! `bench:rust` ran `cargo bench --workspace` against a workspace with no
//! benchmarks in it: zero measured, exit zero, and a task in `vite.config.ts`
//! that had been reporting nothing since it was written. This is what it runs
//! instead.
//!
//! ```sh
//! vp run bench:rust
//! cargo run --release -p slidx_render --example bench -- 500
//! ```
//!
//! # Why not a benchmark framework
//!
//! For the same reason `scripts/bench-build.mjs` is a script rather than one.
//! These numbers are **reported, not gated** — `scripts/budget.mjs` explains
//! why time is the wrong thing to fail a build on, and a harness that fails on
//! a slow afternoon is a harness somebody switches off. What is wanted here is
//! a breakdown a person reads before changing something they expected to be
//! free, and `Instant` gives that without a dependency or a nightly toolchain.
//!
//! The deck is generated rather than read from disk so the figure is about the
//! pipeline rather than about somebody's slides, and every stage is timed
//! separately because a total tells you nothing about what to do next.

use std::hint::black_box;
use std::time::{Duration, Instant};

use slidx_core::{parse_deck, DeckParseOptions};
use slidx_render::shell::{render_slide, ShellOptions};

/// Enough repeats that a cold cache and a scheduling hiccup average out.
const RUNS: u32 = 5;

fn main() {
    let count: usize = std::env::args().nth(1).and_then(|arg| arg.parse().ok()).unwrap_or(500);
    let source = deck_source(count);

    let parse = best(RUNS, || {
        black_box(parse_deck(&source, &DeckParseOptions::default()));
    });

    let deck = parse_deck(&source, &DeckParseOptions::default());
    let options = ShellOptions::default();

    let render = best(RUNS, || {
        for slide in &deck.slides {
            black_box(render_slide(&deck, slide, &options).len());
        }
    });

    let theme = best(RUNS, || {
        black_box(slidx_theme::css::render(&options.theme).len());
    });
    let layouts = best(RUNS, || {
        black_box(slidx_theme::layout::stylesheet().len());
    });

    let bytes: usize =
        deck.slides.iter().map(|slide| render_slide(&deck, slide, &options).len()).sum();

    println!("{count} slides, best of {RUNS}\n");
    println!("  parse            {parse:>12.2?}");
    println!("  render           {render:>12.2?}");
    println!("  ├ theme css      {theme:>12.2?}  (once per deck)");
    println!("  └ layout css     {layouts:>12.2?}  (once per process)");
    println!("\n  emitted          {:>10} kB", bytes / 1000);
    println!("  per slide        {:>12.2?}", render / count as u32);

    by_content(count);
}

/// The same shell, four different slides.
///
/// A total says the render is slow; this says *what* is slow, which is the
/// question anybody reading it actually has. Every row emits within a few
/// hundred bytes of the same document — the shell stylesheet, the theme and the
/// layouts are the same 29kB every time — so the spread between the rows is
/// content processing and nothing else, and an empty slide is the floor:
/// assemble the page and stop.
///
/// The gap is the finding. Assembling a 29kB document costs about 21µs; a
/// realistic slide costs an order of magnitude more, and none of it is the
/// shell. Anybody reaching for the page assembly is optimising the wrong end,
/// which is a mistake this table exists to stop somebody making twice.
fn by_content(count: usize) {
    let cases = [
        ("empty", "# H\n"),
        ("prose", "## Slide\n\nA paragraph of the length prose usually is, twice over.\n"),
        ("prose + code", "## Slide\n\nA paragraph mentioning `inline code` and **emphasis**.\n"),
        ("list + mark + code", REALISTIC),
    ];

    println!("\n  per slide, by what is on it");
    for (label, body) in cases {
        let source = (0..count).map(|_| body.to_string()).collect::<Vec<_>>().join("\n---\n\n");
        let deck = parse_deck(&source, &DeckParseOptions::default());
        let options = ShellOptions::default();
        let bytes = render_slide(&deck, &deck.slides[0], &options).len();

        let taken = best(RUNS, || {
            for slide in &deck.slides {
                black_box(render_slide(&deck, slide, &options).len());
            }
        });

        println!("    {label:<20} {:>9.2?}   {bytes} B", taken / count as u32);
    }
}

fn best(runs: u32, mut work: impl FnMut()) -> Duration {
    (0..runs)
        .map(|_| {
            let started = Instant::now();
            work();
            started.elapsed()
        })
        .min()
        .expect("at least one run")
}

/// A slide of realistic weight: a heading, a list, prose, and a mark.
fn deck_source(count: usize) -> String {
    (0..count)
        .map(|index| REALISTIC.replacen("## Slide", &format!("## Slide {index}"), 1))
        .collect::<Vec<_>>()
        .join("\n---\n\n")
}

/// The slide the headline figure is measured against, and the last row of the
/// breakdown, so the two cannot drift into describing different decks.
const REALISTIC: &str = "## Slide\n\n\
     - The first point\n\
     - The second, with [a marked phrase]{#result .accent}\n\
     - The third\n\n\
     A paragraph of the length prose usually is, mentioning `inline code` and \
     **emphasis** so both reach the highlighter and the theme.\n";
