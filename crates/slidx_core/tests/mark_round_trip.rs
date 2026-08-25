//! The round-trip law for inline marks.
//!
//! slidx claims the canvas and the Markdown file are two views of one
//! document. That claim is only worth making if it is mechanised, so this file
//! states it as three properties and checks them over generated input rather
//! than over a handful of examples.
//!
//! 1. **Serialising is canonical.** Two marks meaning the same thing produce
//!    the same source, so opening a deck in the editor and closing it cannot
//!    rewrite lines the author did not touch.
//! 2. **Parsing inverts serialising.** Anything the editor can model, it can
//!    write and read back unchanged.
//! 3. **Serialising is idempotent.** A hand-written mark survives a trip
//!    through the editor without drifting.
//!
//! Property three is the one that matters to a person: it is the difference
//! between a tool you can hand-edit and a tool that owns your file.

use slidx_core::mark::{find_marks, Mark};

/// A deterministic pseudo-random generator.
///
/// Hand-rolled so the property tests need no dependency and no `Date::now`:
/// the same seed produces the same cases on every machine and every run, which
/// is what makes a property failure reproducible.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn pick<'a, T>(&mut self, options: &'a [T]) -> &'a T {
        &options[(self.next() % options.len() as u64) as usize]
    }

    fn chance(&mut self, one_in: u64) -> bool {
        self.next().is_multiple_of(one_in)
    }
}

/// Text fragments chosen to include everything that needs escaping or is
/// otherwise awkward: brackets, braces, quotes, CJK, and emoji.
const TEXTS: &[&str] = &[
    "selected words",
    "a",
    "日本語のテキスト",
    "with [brackets] inside",
    "with \"quotes\"",
    "with \\backslash",
    "trailing space ",
    "🚀 emoji",
    "= equals { braces }",
];

const KEYS: &[&str] = &["hero", "point-1", "a", "見出し"];
const CLASSES: &[&str] = &["accent", "muted", "code"];
const NAMES: &[&str] = &["color", "font", "size", "opacity"];
const VALUES: &[&str] = &["danger", "mono", "lg", "0.5", "two words", "", "with\"quote"];

fn generate(rng: &mut Rng) -> Mark {
    let mut mark = Mark::new(*rng.pick(TEXTS));

    if !rng.chance(3) {
        mark = mark.with_key(*rng.pick(KEYS));
    }
    for _ in 0..(rng.next() % 3) {
        mark = mark.with_class(*rng.pick(CLASSES));
    }
    for _ in 0..(rng.next() % 3) {
        mark = mark.with_property(*rng.pick(NAMES), *rng.pick(VALUES));
    }

    // A mark with nothing on it is plain text, not a mark; the editor drops
    // those, so they are not part of the round-trip domain.
    if mark.is_bare() {
        mark = mark.with_class("accent");
    }

    mark
}

/// Parses a source string that should hold exactly one mark.
fn parse_one(source: &str) -> Option<Mark> {
    let found = find_marks(source);
    (found.len() == 1 && found[0].start == 0 && found[0].end == source.len())
        .then(|| found[0].mark.clone())
}

#[test]
fn serialising_then_parsing_returns_the_same_mark() {
    let mut rng = Rng(0x5115_D000_0000_0001);

    for case in 0..2000 {
        let mark = generate(&mut rng);
        let source = mark.to_source();

        let parsed = parse_one(&source)
            .unwrap_or_else(|| panic!("case {case}: {source:?} did not parse as one mark"));

        assert_eq!(parsed, mark, "case {case}: {source:?} round-tripped to a different mark");
    }
}

#[test]
fn serialising_is_idempotent() {
    let mut rng = Rng(0x5115_D000_0000_0002);

    for case in 0..2000 {
        let once = generate(&mut rng).to_source();
        let twice = parse_one(&once)
            .unwrap_or_else(|| panic!("case {case}: {once:?} did not parse"))
            .to_source();

        assert_eq!(once, twice, "case {case}: a second save rewrote the line");
    }
}

#[test]
fn equal_marks_serialise_identically_whatever_order_they_were_built_in() {
    // Property attributes are stored sorted, so an editor that applies colour
    // before font produces the same line as one that applies font first.
    let a = Mark::new("text")
        .with_key("k")
        .with_property("color", "danger")
        .with_property("font", "mono");
    let b = Mark::new("text")
        .with_key("k")
        .with_property("font", "mono")
        .with_property("color", "danger");

    assert_eq!(a.to_source(), b.to_source());
    assert_eq!(a, b);
}

#[test]
fn a_mark_with_no_attributes_serialises_as_plain_text() {
    assert_eq!(Mark::new("plain").to_source(), "plain");
    assert!(find_marks("plain").is_empty());
}

#[test]
fn a_link_is_not_a_mark() {
    // The distinction is `]{`, which CommonMark leaves undefined, so claiming
    // it cannot change the meaning of an existing document.
    assert!(find_marks("see [the docs](https://example.com)").is_empty());
    assert!(find_marks("![alt](./a.png)").is_empty());
}

#[test]
fn an_escaped_bracket_is_not_a_mark() {
    assert!(find_marks("\\[not a mark]{.accent}").is_empty());
}

#[test]
fn an_unterminated_mark_is_left_as_text() {
    // Half-typed marks exist constantly while someone is editing. None of them
    // may make the rest of the slide vanish.
    assert!(find_marks("[unclosed").is_empty());
    assert!(find_marks("[text]{unclosed").is_empty());
    assert!(find_marks("[text]").is_empty());
}

#[test]
fn several_marks_in_one_paragraph_are_found_in_order() {
    let found = find_marks("A [one]{#a} and [two]{#b} here.");

    assert_eq!(found.len(), 2);
    assert_eq!(found[0].mark.key.as_deref(), Some("a"));
    assert_eq!(found[1].mark.key.as_deref(), Some("b"));
    assert!(found[0].end <= found[1].start);
}

#[test]
fn ranges_point_at_the_exact_source_the_mark_occupies() {
    let source = "A [one]{#a} here.";
    let found = &find_marks(source)[0];

    assert_eq!(&source[found.start..found.end], "[one]{#a}");
}

#[test]
fn the_attribute_group_is_located_apart_from_the_text_it_annotates() {
    // What lets a formatter reorder attributes without re-emitting the marked
    // text. The text is prose: serialising it would escape brackets the author
    // typed on purpose, which is the diff the whole round-trip law exists to
    // prevent.
    let source = "A [a [nested] b]{color=danger #k .accent} here.";
    let found = &find_marks(source)[0];

    assert_eq!(&source[found.attributes_start..found.end], "{color=danger #k .accent}");
    assert_eq!(found.mark.attributes_source(), "#k .accent color=danger");
}

#[test]
fn the_attribute_list_is_the_part_of_a_canonical_mark_inside_the_braces() {
    // Two callers, one order. A formatter that built its own list could sort
    // properties differently from the editor, and the two would then fight over
    // the same line every time they took turns.
    let mut rng = Rng(0x5115_D000_0000_0003);

    for case in 0..500 {
        let mark = generate(&mut rng);
        let group = format!("{{{}}}", mark.attributes_source());

        assert!(mark.to_source().ends_with(&group), "case {case}: {group} is not how it ends");
    }
}

#[test]
fn nested_brackets_inside_the_text_are_balanced() {
    let found = find_marks("[a [nested] b]{#k}");

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].mark.text, "a [nested] b");
}

#[test]
fn a_bare_word_is_shorthand_for_a_class() {
    let found = find_marks("[text]{accent}");
    assert_eq!(found[0].mark.classes, vec!["accent".to_string()]);
}

#[test]
fn quoted_values_keep_their_spaces() {
    let found = find_marks("[text]{caption=\"two words\"}");
    assert_eq!(found[0].mark.properties["caption"], "two words");
}

#[test]
fn a_key_gives_the_mark_a_selector_for_steps() {
    let mark = Mark::new("text").with_key("hero");
    assert_eq!(mark.selector().as_deref(), Some("[data-slidx-mark=\"hero\"]"));
    assert!(Mark::new("text").with_class("accent").selector().is_none());
}
