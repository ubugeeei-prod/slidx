//! The box, and what goes inside it.
//!
//! Pure: a slide, a stop and a size in, a frame of text out. The loop in
//! [`super`] does nothing but read a key and print what this returns, which is
//! what makes every interesting case — a slide that overflows, a stop that
//! reveals nothing, a deck with one slide — a line of test setup rather than a
//! terminal somebody has to sit in front of.
//!
//! ## Why a box, and why that shape
//!
//! A slide is a fixed rectangle and a terminal is not. Filling the terminal
//! would show a deck reflowed to whatever window happens to be open, which is
//! the one thing a slide never does — and it would quietly suggest the content
//! fits when the only thing being demonstrated is that it fits *here*.
//!
//! So the box is drawn at the deck's own declared aspect ratio, and the
//! calculation accounts for the terminal cell being about twice as tall as it
//! is wide. A 16:9 box drawn as 16:9 *cells* is not 16:9 to look at; it is
//! nearly square, and it would make a wide deck look like a narrow one.
//!
//! ## The box is not a promise
//!
//! Content that does not fit the box is **not** evidence that it will not fit
//! the slide, and content that does fit is not evidence that it will. A
//! terminal row is not a line of 40pt type. The frame says so on screen, every
//! time, in the footer — see [`Frame::footer`] — because the failure worth
//! designing against is somebody checking here, seeing it fit, and finding out
//! on stage.

use slidx_core::{AspectRatio, Deck, Slide};

use super::outline::{self, Kind, Line};
use crate::style::{self, Ink, Style};

/// How wide a terminal cell is relative to its height.
///
/// Terminals do not report this and it varies a little by font, but every
/// monospace font in use is close to 1:2. Getting it wrong in either direction
/// misrepresents the deck's shape, which is most of what the box is for.
const CELL_ASPECT: f64 = 0.5;

/// Never draw a box narrower than this; below it nothing is legible anyway.
const MIN_WIDTH: usize = 24;

/// The size of the box, in terminal cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Box2 {
    pub width: usize,
    pub height: usize,
}

impl Box2 {
    /// The largest box of `aspect` that fits in `columns` by `rows`.
    ///
    /// Width-first, because a terminal is nearly always wider than it is tall
    /// once the cell shape is accounted for; the height check is what catches
    /// a short window and a 4:3 deck.
    pub fn fitting(aspect: AspectRatio, columns: usize, rows: usize) -> Self {
        let (wide, tall) = aspect.dimensions();
        let ratio = wide as f64 / tall as f64;

        let width = columns.max(MIN_WIDTH);
        let height = ((width as f64 * CELL_ASPECT) / ratio).round().max(3.0) as usize;

        if height <= rows {
            return Self { width, height };
        }

        // Too tall for the window: give up width until it fits.
        let height = rows.max(3);
        let width = ((height as f64 * ratio) / CELL_ASPECT).round().max(MIN_WIDTH as f64) as usize;

        Self { width: width.min(columns.max(MIN_WIDTH)), height }
    }

    /// Rows available for content, once the border has taken two.
    pub fn inner_height(self) -> usize {
        self.height.saturating_sub(2)
    }

    /// Columns available for content, once the border and padding have taken
    /// two each.
    pub fn inner_width(self) -> usize {
        self.width.saturating_sub(4)
    }
}

/// Everything one frame of the view depends on.
#[derive(Debug, Clone, Copy)]
pub struct View<'a> {
    pub deck: &'a Deck,
    pub slide: usize,
    pub stop: usize,
    pub size: Box2,
}

/// Draws one frame.
pub fn frame(view: &View<'_>, style: &Style) -> String {
    let Some(slide) = view.deck.slides.get(view.slide) else {
        return String::new();
    };

    let mut text = String::new();
    text.push_str(&rule(view.size.width, '-', style));

    let lines = outline::lines(slide, view.stop);
    let room = view.size.inner_height();
    let drawn = lines.len().min(room);

    for line in &lines[..drawn] {
        text.push_str(&row(&render(line, view.size.inner_width(), style), view.size.width, style));
    }

    // What did not fit is said out loud rather than silently cut. A view that
    // hid the last three bullets would be worse than one that showed none.
    if lines.len() > room {
        let over = lines.len() - room;
        text.push_str(&row(
            &style.paint(Ink::Warn, format!("... {over} more line(s) below the box")),
            view.size.width,
            style,
        ));
    } else {
        for _ in drawn..room {
            text.push_str(&row("", view.size.width, style));
        }
    }

    text.push_str(&rule(view.size.width, '-', style));
    text.push_str(&status(view, slide, style));
    text.push_str(&footer(style));

    text
}

/// One line of a slide, with its markdown furniture put back as indentation.
///
/// The text is cut to the box **before** it is styled. Cutting afterwards
/// would slice through an escape sequence and leave the rest of the frame
/// wearing whatever colour was half-applied.
fn render(line: &Line, width: usize, style: &Style) -> String {
    // A hidden mark keeps its width so the slide does not move between stops,
    // while saying nothing about what it will contain. Its width is the cells
    // the text would have taken, so a Japanese mark does not shrink the slide
    // by half when it is hidden and grow it again when it is revealed.
    if line.hidden {
        return style.paint(Ink::Faint, ".".repeat(style::width::of(&line.text).min(width)));
    }

    let (ink, plain) = match line.kind {
        Kind::Heading(level) => (
            Ink::Strong,
            format!("{}{}", " ".repeat((level as usize).saturating_sub(1) * 2), line.text),
        ),
        Kind::Bullet(depth) => {
            (Ink::Strong, format!("{}- {}", " ".repeat(depth as usize * 2), line.text))
        }
        Kind::Ordered(depth) => {
            (Ink::Strong, format!("{}1. {}", " ".repeat(depth as usize * 2), line.text))
        }
        Kind::Quote => (Ink::Faint, format!("| {}", line.text)),
        Kind::Code => (Ink::Faint, line.text.clone()),
        Kind::Fence => (Ink::Faint, "-".repeat(width.min(12))),
        Kind::Rule => (Ink::Faint, "-".repeat(width)),
        Kind::Text | Kind::Blank => (Ink::Strong, line.text.clone()),
    };

    let cut = truncate(&plain, width);

    match line.kind {
        // Code is handed to the highlighter after the cut, so what it colours
        // is exactly what is drawn.
        Kind::Code => super::code::render(&cut, line.language.as_deref(), style),
        Kind::Text | Kind::Blank | Kind::Bullet(_) | Kind::Ordered(_) => cut,
        _ => style.paint(ink, cut),
    }
}

/// Cuts a line to the box, marking that it continues.
///
/// The marker matters more than the cut: a line that simply stopped would read
/// as a line that ends there, and this view is looked at by somebody counting
/// what is on a slide.
fn truncate(text: &str, width: usize) -> String {
    if style::width::of(text) <= width {
        return text.to_string();
    }

    // Cells, not characters: a slide of Japanese cut by character count runs
    // through the right-hand border of its own box.
    format!("{}>", style::width::clip(text, width.saturating_sub(1)))
}

/// The line under the box: where you are, and what the stop does.
fn status(view: &View<'_>, slide: &Slide, style: &Style) -> String {
    let stops = slide.timeline.len();
    let title = slide.title.clone().unwrap_or_else(|| "untitled".to_string());

    let position = format!(
        "slide {}/{}  stop {}/{}",
        view.slide + 1,
        view.deck.slides.len(),
        view.stop + 1,
        stops
    );

    let room = style::WIDTH.saturating_sub(style::width::of(&position) + 4);
    let short = if style::width::of(&title) > room {
        format!("{}...", style::width::clip(&title, room.saturating_sub(3)))
    } else {
        title
    };

    format!("  {}  {}\n", style.paint(Ink::Strong, &position), style.paint(Ink::Faint, &short))
}

/// The one sentence that has to be on screen every time.
///
/// Not in the help behind a key — on the frame. Somebody who checks a deck
/// here, sees it fit, and finds out on stage that it does not is a failure this
/// tool caused, and a disclaimer nobody opened does not prevent it.
pub fn footer(style: &Style) -> String {
    format!(
        "  {}\n",
        style.paint(
            Ink::Faint,
            "structure and flow only — never appearance. ? for keys, q to quit."
        )
    )
}

fn rule(width: usize, character: char, style: &Style) -> String {
    format!("  {}\n", style.paint(Ink::Faint, character.to_string().repeat(width)))
}

/// One row inside the border, padded to the box width.
fn row(body: &str, width: usize, style: &Style) -> String {
    let visible = visible_width(body);
    let room = width.saturating_sub(4);
    let padding = room.saturating_sub(visible.min(room));

    format!(
        "  {}{}{}{}\n",
        style.paint(Ink::Faint, "|"),
        format_args!(" {body}"),
        " ".repeat(padding),
        style.paint(Ink::Faint, " |")
    )
}

/// Cells a terminal will actually draw.
///
/// [`crate::style::width`] is the one answer to this question in the whole
/// binary. A second implementation here is how the box ends up one column wider
/// than the report beside it on a Japanese slide.
fn visible_width(text: &str) -> usize {
    style::width::of(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn deck(source: &str) -> Deck {
        parse_deck(source, &DeckParseOptions::default())
    }

    fn view<'a>(deck: &'a Deck, slide: usize, stop: usize) -> View<'a> {
        View { deck, slide, stop, size: Box2 { width: 60, height: 12 } }
    }

    #[test]
    fn a_sixteen_by_nine_box_is_wide_rather_than_nearly_square() {
        // Drawn as 16:9 cells it would be nearly square, and a wide deck would
        // look like a narrow one. The cell being twice as tall as it is wide is
        // the whole calculation.
        let size = Box2::fitting(AspectRatio::default(), 80, 40);

        assert_eq!(size.width, 80);
        // 80 * 0.5 / (16/9) = 22.5
        assert_eq!(size.height, 23);
    }

    #[test]
    fn a_four_by_three_deck_gets_a_taller_box_than_a_widescreen_one() {
        let wide = Box2::fitting(AspectRatio::parse("16:9").expect("16:9"), 80, 60);
        let square = Box2::fitting(AspectRatio::parse("4:3").expect("4:3"), 80, 60);

        assert!(square.height > wide.height, "{square:?} vs {wide:?}");
    }

    #[test]
    fn a_box_too_tall_for_the_window_gives_up_width_rather_than_shape() {
        // The alternative is a box that is not the deck's shape, which is the
        // one thing it exists to be.
        let size = Box2::fitting(AspectRatio::parse("4:3").expect("4:3"), 200, 20);

        assert!(size.height <= 20, "{size:?}");
        assert!(size.width < 200, "{size:?}");
    }

    #[test]
    fn a_very_narrow_terminal_still_gets_a_box() {
        // Unreadable, but a panic or a zero-width frame is worse.
        let size = Box2::fitting(AspectRatio::default(), 5, 3);

        assert!(size.width >= MIN_WIDTH);
        assert!(size.height >= 3);
    }

    #[test]
    fn a_heading_is_drawn_without_its_hashes() {
        let deck = deck("## A heading\n");
        let text = frame(&view(&deck, 0, 0), &Style::plain());

        assert!(text.contains("A heading"), "{text}");
        assert!(!text.contains("##"), "{text}");
    }

    #[test]
    fn a_bullet_is_drawn_with_a_marker_and_its_nesting() {
        let deck = deck("- one\n  - two\n");
        let text = frame(&view(&deck, 0, 0), &Style::plain());

        assert!(text.contains("- one"), "{text}");
        assert!(text.contains("  - two"), "{text}");
    }

    #[test]
    fn a_mark_a_step_has_not_revealed_is_drawn_as_a_placeholder_of_its_width() {
        // The shape of the slide has to stay still between stops, and the
        // placeholder must not claim to be the text.
        let deck = deck("---\nsteps:\n  - reveal: \"#later\"\n---\n\n- [not yet]{#later}\n");
        let hidden = frame(&view(&deck, 0, 0), &Style::plain());
        let shown = frame(&view(&deck, 0, 1), &Style::plain());

        assert!(hidden.contains(".."), "{hidden}");
        assert!(!hidden.contains("not yet"), "{hidden}");
        assert!(shown.contains("not yet"), "{shown}");
        assert_eq!(hidden.lines().count(), shown.lines().count());
    }

    #[test]
    fn the_status_line_says_where_in_the_deck_and_where_in_the_slide() {
        let deck = deck("# One\n\n---\n\n# Two\n");
        let text = frame(&view(&deck, 1, 0), &Style::plain());

        assert!(text.contains("slide 2/2"), "{text}");
        assert!(text.contains("stop 1/1"), "{text}");
    }

    #[test]
    fn content_that_does_not_fit_is_reported_rather_than_silently_cut() {
        // A view that quietly dropped the last three bullets would be worse
        // than one that showed none of them.
        let deck = deck(&format!("# One\n\n{}", "- bullet\n".repeat(40)));
        let text = frame(&view(&deck, 0, 0), &Style::plain());

        assert!(text.contains("more line(s) below the box"), "{text}");
    }

    #[test]
    fn a_short_slide_is_padded_so_the_box_keeps_its_shape() {
        let deck = deck("# One\n");
        let text = frame(&view(&deck, 0, 0), &Style::plain());

        // Two rules, ten inner rows, a status line and a footer.
        assert_eq!(text.lines().count(), 12 + 2);
    }

    #[test]
    fn every_frame_says_it_shows_structure_and_flow_and_never_appearance() {
        // On the frame, not behind a key. Somebody who checks a deck here,
        // sees it fit, and finds out on stage is a failure this tool caused,
        // and a disclaimer nobody opened does not prevent it.
        let deck = deck("# One\n");
        let text = frame(&view(&deck, 0, 0), &Style::plain());

        assert!(text.contains("structure and flow only"), "{text}");
        assert!(text.contains("never appearance"), "{text}");
    }

    #[test]
    fn the_box_is_the_same_width_on_every_row_coloured_or_not() {
        // Escape codes are zero-width on screen. Counted as characters they
        // shear the right-hand border one row at a time.
        let deck = deck("# One\n\n- a bullet\n\n```rust\nfn main() {}\n```\n");

        for style in [Style::plain(), Style::colored()] {
            let text = frame(&view(&deck, 0, 0), &style);
            let widths: Vec<usize> = text
                .lines()
                .filter(|line| line.trim_start().starts_with('|'))
                .map(visible_width)
                .collect();

            assert!(widths.windows(2).all(|pair| pair[0] == pair[1]), "{widths:?}");
        }
    }

    #[test]
    fn a_slide_index_past_the_end_draws_nothing_rather_than_panicking() {
        let deck = deck("# One\n");

        assert!(frame(&view(&deck, 99, 0), &Style::plain()).is_empty());
    }

    #[test]
    fn a_line_wider_than_the_box_does_not_widen_it() {
        let deck = deck(&format!("- {}\n", "x".repeat(300)));
        let text = frame(&view(&deck, 0, 0), &Style::plain());

        for line in text.lines().filter(|line| line.trim_start().starts_with('|')) {
            assert!(visible_width(line) <= 60 + 2, "{}: {line}", visible_width(line));
        }
    }
}
