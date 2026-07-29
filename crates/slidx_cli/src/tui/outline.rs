//! A slide, as lines a terminal can draw.
//!
//! This reads the parsed model — headings, bullets, code blocks, the marks a
//! step addresses — and never the rendered HTML. Rendering the HTML would mean
//! a second renderer, which is the thing this repository refuses to have; and
//! it would also be pretending, because a terminal cannot show what a browser
//! does with that HTML and should not imply it can.
//!
//! ## What this can and cannot tell you
//!
//! It tells you **structure and flow**: how many stops a slide has, what each
//! one reveals, whether the bullets are eight when you thought they were four,
//! whether a heading level was skipped, how the deck reads end to end.
//!
//! It cannot tell you anything about **appearance**. Not whether the text fits,
//! not the contrast, not the font, not the layout. Those are `slidx lint` and
//! the browser, and the reason this file says so at the top is that somebody
//! checking a deck here, seeing it fit, and discovering on stage that it does
//! not, would be a failure this tool caused.

use slidx_core::{Slide, Visibility};

/// What a line of a slide is, so the drawing can style it without re-parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Heading(u8),
    Bullet(u8),
    Ordered(u8),
    Quote,
    /// A line inside a fenced block. The language is on [`Line::language`].
    Code,
    /// The fence itself, which is drawn as a rule rather than as backticks.
    Fence,
    Rule,
    Text,
    Blank,
}

/// One line, ready to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub kind: Kind,
    /// The text without its markdown furniture — `## ` and `- ` are carried by
    /// [`Kind`] instead, so the drawing can render them however it likes.
    pub text: String,
    /// The language of the fenced block this line is inside.
    pub language: Option<String>,
    /// Selectors the compiler left on this line, so a step's visibility
    /// applies without matching on text.
    pub targets: Vec<String>,
    /// True when a step hides one of this line's marks at the stop being shown.
    pub hidden: bool,
}

impl Line {
    fn new(kind: Kind, text: impl Into<String>) -> Self {
        Self { kind, text: text.into(), language: None, targets: Vec::new(), hidden: false }
    }
}

/// Turns one slide into lines, at one stop.
///
/// `stop` is an index into the slide's timeline. Out of range is clamped rather
/// than refused: a caller that has just narrowed a deck should get the last
/// stop, not a panic in the middle of a redraw.
pub fn lines(slide: &Slide, stop: usize) -> Vec<Line> {
    let hidden = hidden_targets(slide, stop);
    let mut out = Vec::new();
    let mut fence: Option<String> = None;

    for raw in slide.content.lines() {
        let trimmed = raw.trim_end();

        if let Some(language) = fence_at(trimmed) {
            // A fence toggles. The opening one carries the language; the
            // closing one carries nothing and must not start a second block.
            out.push(Line::new(Kind::Fence, ""));
            fence = match fence {
                Some(_) => None,
                None => Some(language),
            };
            continue;
        }

        if let Some(language) = &fence {
            let mut line = Line::new(Kind::Code, trimmed);
            line.language = Some(language.clone());
            out.push(line);
            continue;
        }

        let (text, targets) = strip_markers(trimmed);
        let mut line = classify(&text);
        line.targets = targets;
        out.push(line);
    }

    mark_hidden(&mut out, &hidden);
    trim_edges(out)
}

/// One line of prose, by what it starts with.
fn classify(line: &str) -> Line {
    let text = line.trim_start();
    let indent = (line.len() - text.len()) as u8;

    if text.is_empty() {
        return Line::new(Kind::Blank, "");
    }

    if text.starts_with("---") || text.starts_with("***") || text.starts_with("___") {
        return Line::new(Kind::Rule, "");
    }

    if let Some(rest) = text.strip_prefix('>') {
        return Line::new(Kind::Quote, rest.trim_start());
    }

    let hashes = text.chars().take_while(|c| *c == '#').count();
    if hashes > 0 && hashes <= 6 && text.chars().nth(hashes) == Some(' ') {
        return Line::new(Kind::Heading(hashes as u8), text[hashes + 1..].trim());
    }

    for bullet in ["- ", "* ", "+ "] {
        if let Some(rest) = text.strip_prefix(bullet) {
            // Two spaces to a level is the markdown convention; anything
            // deeper still reads as nested rather than as a new list.
            return Line::new(Kind::Bullet(indent / 2), rest.trim_start());
        }
    }

    if let Some(rest) = ordered(text) {
        return Line::new(Kind::Ordered(indent / 2), rest);
    }

    Line::new(Kind::Text, text)
}

/// `1. text`, returning what follows the marker.
fn ordered(text: &str) -> Option<&str> {
    let digits = text.chars().take_while(char::is_ascii_digit).count();

    if digits == 0 {
        return None;
    }

    text[digits..].strip_prefix(". ").map(str::trim_start)
}

/// The language on an opening fence, or `None` for a line that is not one.
fn fence_at(line: &str) -> Option<String> {
    let text = line.trim_start();

    text.strip_prefix("```")
        .or_else(|| text.strip_prefix("~~~"))
        .map(|language| language.trim().to_string())
}

/// Takes the compiler's own spans out of a line, returning the text and the
/// selectors that were on it.
///
/// Two kinds end up in a slide's content, and both are machinery rather than
/// words somebody wrote:
///
/// - `[text]{#key}` becomes `<span data-slidx-mark="key">text</span>`;
/// - a `<!-- step -->` marker becomes an empty
///   `<span data-slidx-step="1" hidden></span>` left where it stood.
///
/// The selector each carries — `[data-slidx-mark="key"]`,
/// `[data-slidx-step="1"]` — is exactly what the compiled timeline addresses,
/// so this hands back the same strings the frame is keyed on rather than
/// reconstructing them.
///
/// Only slidx's own spans are removed. HTML the author wrote is left as
/// written, because guessing which of somebody's tags were decorative is how a
/// preview starts lying about the source.
fn strip_markers(line: &str) -> (String, Vec<String>) {
    const OPEN: &str = "<span data-slidx-";
    const CLOSE: &str = "</span>";

    let mut text = String::with_capacity(line.len());
    let mut targets = Vec::new();
    let mut rest = line;

    while let Some(at) = rest.find(OPEN) {
        text.push_str(&rest[..at]);
        let after = &rest[at + "<span ".len()..];

        // `data-slidx-mark="key"` or `data-slidx-step="1"`, up to the quote
        // that closes the value.
        let Some(equals) = after.find('=') else { break };
        let attribute = &after[..equals];
        let value_start = equals + 2;
        let Some(quote) = after[value_start..].find('"') else { break };
        let value = &after[value_start..value_start + quote];

        targets.push(format!("[{attribute}=\"{value}\"]"));

        // Past the rest of the attributes to the end of the opening tag.
        let Some(close) = after[value_start + quote..].find('>') else { break };
        rest = &after[value_start + quote + close + 1..];

        // A mark wraps its text; a step anchor is empty. Both end the same way.
        let Some(end) = rest.find(CLOSE) else { break };
        text.push_str(&rest[..end]);
        rest = &rest[end + CLOSE.len()..];
    }

    text.push_str(rest);
    (text, targets)
}

/// The selectors a step has hidden at this stop.
///
/// Read straight off the compiled frame, so what is hidden here is what the
/// runtime hides on the projector rather than a second guess at the same
/// question.
fn hidden_targets(slide: &Slide, stop: usize) -> Vec<String> {
    let index = stop.min(slide.timeline.last_index());
    let Some(frame) = slide.timeline.frame(index) else {
        return Vec::new();
    };

    frame
        .states
        .iter()
        .filter(|state| state.visibility == Visibility::Hidden)
        .map(|state| state.target.clone())
        .collect()
}

/// Flags the lines carrying text a step has not revealed yet.
///
/// Flagged rather than removed: the drawing shows a placeholder of the same
/// width, so the shape of the slide does not move between stops. A slide that
/// reflowed on every press would misrepresent the one thing this view is for.
fn mark_hidden(lines: &mut [Line], hidden: &[String]) {
    if hidden.is_empty() {
        return;
    }

    for line in lines.iter_mut() {
        if line.targets.iter().any(|target| hidden.contains(target)) {
            line.hidden = true;
        }
    }
}

/// Drops blank lines at the top and bottom.
///
/// A slide's content usually begins and ends with one, and a box drawn around
/// them wastes two of the very few rows a terminal has.
fn trim_edges(mut lines: Vec<Line>) -> Vec<Line> {
    while lines.first().is_some_and(|line| line.kind == Kind::Blank) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.kind == Kind::Blank) {
        lines.pop();
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn slide(source: &str) -> Slide {
        parse_deck(source, &DeckParseOptions::default()).slides.into_iter().next().expect("a slide")
    }

    fn kinds(source: &str) -> Vec<Kind> {
        lines(&slide(source), 0).iter().map(|line| line.kind).collect()
    }

    fn texts(source: &str) -> Vec<String> {
        lines(&slide(source), 0).iter().map(|line| line.text.clone()).collect()
    }

    #[test]
    fn a_heading_carries_its_level_and_loses_its_hashes() {
        // The level is what a reader is checking — a skipped one is a real
        // problem — and the hashes are furniture the drawing supplies itself.
        assert_eq!(kinds("### Third level\n"), [Kind::Heading(3)]);
        assert_eq!(texts("### Third level\n"), ["Third level"]);
    }

    #[test]
    fn something_that_only_looks_like_a_heading_is_not_one() {
        // `#hashtag` and a row of hashes are both text.
        assert_eq!(kinds("#hashtag\n"), [Kind::Text]);
        assert_eq!(kinds("####### seven\n"), [Kind::Text]);
    }

    #[test]
    fn every_bullet_marker_markdown_allows_is_a_bullet() {
        assert_eq!(kinds("- one\n"), [Kind::Bullet(0)]);
        assert_eq!(kinds("* one\n"), [Kind::Bullet(0)]);
        assert_eq!(kinds("+ one\n"), [Kind::Bullet(0)]);
    }

    #[test]
    fn a_nested_bullet_carries_its_depth() {
        // Counting the levels is half of what somebody is here to do.
        assert_eq!(
            kinds("- one\n  - two\n    - three\n"),
            [Kind::Bullet(0), Kind::Bullet(1), Kind::Bullet(2)]
        );
    }

    #[test]
    fn a_numbered_list_is_told_apart_from_a_sentence_beginning_with_a_number() {
        assert_eq!(kinds("1. first\n"), [Kind::Ordered(0)]);
        assert_eq!(texts("1. first\n"), ["first"]);
        assert_eq!(kinds("1994 was a year\n"), [Kind::Text]);
    }

    #[test]
    fn a_fenced_block_is_code_until_it_closes() {
        let kinds = kinds("```rust\nfn main() {}\n```\n\nafter\n");

        assert_eq!(kinds, [Kind::Fence, Kind::Code, Kind::Fence, Kind::Blank, Kind::Text]);
    }

    #[test]
    fn every_line_of_a_block_carries_the_language_the_fence_declared() {
        // Which is what the highlighter is handed. A line without it would be
        // drawn as plain text in the middle of coloured code.
        let lines = lines(&slide("```rust\nfn a() {}\nfn b() {}\n```\n"), 0);
        let code: Vec<Option<&str>> = lines
            .iter()
            .filter(|line| line.kind == Kind::Code)
            .map(|line| line.language.as_deref())
            .collect();

        assert_eq!(code, [Some("rust"), Some("rust")]);
    }

    #[test]
    fn a_fence_with_no_language_is_still_a_block() {
        let lines = lines(&slide("```\nplain\n```\n"), 0);

        assert_eq!(lines[1].kind, Kind::Code);
        assert_eq!(lines[1].language.as_deref(), Some(""));
    }

    #[test]
    fn markdown_inside_a_code_block_is_not_read_as_markdown() {
        // The oldest bug in every markdown renderer: `# not a heading` inside
        // a fence.
        let kinds = kinds("```sh\n# not a heading\n- not a bullet\n```\n");

        assert_eq!(kinds, [Kind::Fence, Kind::Code, Kind::Code, Kind::Fence]);
    }

    #[test]
    fn a_tilde_fence_works_the_same_as_a_backtick_one() {
        assert_eq!(kinds("~~~js\nlet x = 1;\n~~~\n"), [Kind::Fence, Kind::Code, Kind::Fence]);
    }

    #[test]
    fn a_blockquote_keeps_its_text_and_loses_its_marker() {
        assert_eq!(kinds("> quoted\n"), [Kind::Quote]);
        assert_eq!(texts("> quoted\n"), ["quoted"]);
    }

    #[test]
    fn blank_lines_at_the_edges_are_dropped_and_the_ones_between_are_kept() {
        // The box has very few rows. Two of them spent on the blank lines a
        // slide happens to begin and end with is two too many.
        let kinds = kinds("\n\n# One\n\ntext\n\n\n");

        assert_eq!(kinds, [Kind::Heading(1), Kind::Blank, Kind::Text]);
    }

    #[test]
    fn a_slide_with_nothing_on_it_produces_no_lines_rather_than_a_blank_one() {
        assert!(lines(&slide("\n"), 0).is_empty());
    }

    #[test]
    fn a_mark_a_step_has_not_revealed_yet_is_flagged_hidden() {
        // What stepping is for: the difference between one stop and the next
        // has to be visible.
        let deck = parse_deck(
            "---\nsteps:\n  - reveal: \"#later\"\n---\n\n# One\n\n- always here\n- [not yet]{#later}\n",
            &DeckParseOptions::default(),
        );
        let slide = deck.slides.first().expect("a slide");

        let first = lines(slide, 0);
        let second = lines(slide, 1);

        assert!(first.iter().any(|line| line.hidden), "nothing was hidden at the first stop");
        assert!(second.iter().all(|line| !line.hidden), "still hidden after the reveal");
    }

    #[test]
    fn a_hidden_line_keeps_its_text_so_the_slide_does_not_reflow() {
        // Flagged rather than removed. A slide whose shape moved on every
        // press would misrepresent the one thing this view is for.
        let deck = parse_deck(
            "---\nsteps:\n  - reveal: \"#later\"\n---\n\n- [not yet]{#later}\n",
            &DeckParseOptions::default(),
        );
        let slide = deck.slides.first().expect("a slide");
        let first = lines(slide, 0);

        assert_eq!(first.len(), lines(slide, 1).len());
        assert!(!first[0].text.is_empty());
    }

    #[test]
    fn a_stop_past_the_end_shows_the_last_one_rather_than_panicking() {
        // Reachable in the middle of a redraw when a caller has just moved to
        // a shorter slide.
        let slide = slide("# One\n");

        assert_eq!(lines(&slide, 999), lines(&slide, 0));
    }

    #[test]
    fn a_slide_with_no_marks_hides_nothing_however_many_stops_it_has() {
        let slide = slide("# One\n\n- a\n- b\n");

        assert!(lines(&slide, 0).iter().all(|line| !line.hidden));
    }
}
