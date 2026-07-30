//! What changed, said in slides rather than in lines.
//!
//! This is the whole reason `slidx save` is not an alias for `git commit -a`.
//! git compares two files and can say `+34 -6`; slidx has a parser, so it can
//! say *two slides added, one dropped, the demo retimed*. That is the sentence
//! the author would have typed, and the one they will want to read in a year
//! when they are looking for the version of the talk that had the live demo in
//! it.
//!
//! It lives beside the deck model rather than beside the command that first
//! needed it, because two things ask this question and they must not answer it
//! differently: `slidx save` writes the sentence into a commit message, and the
//! editor's history panel shows it against a commit that already exists. A
//! second implementation would let a talk's history and its record of itself
//! disagree about what an author did.
//!
//! ## Matching slides across a change
//!
//! There is no identity in a Markdown file — a slide is bytes between
//! separators, and moving one leaves no trace of where it was. So slides are
//! paired in three passes, weakest evidence last:
//!
//! 1. **Identical.** Same title and same body: certainly the same slide,
//!    wherever it now sits. This is what makes a reorder read as a reorder
//!    rather than as everything having been rewritten.
//! 2. **Same id.** The slug in the URL, which survives an edit to the body.
//! 3. **Same position among what is left.** A slide edited *and* retitled has
//!    nothing else to go on, and the k-th unmatched slide before is the k-th
//!    unmatched slide after far more often than not.
//!
//! Whatever is unpaired at the end is genuinely added or dropped.
//!
//! ## Nothing is appended to a message
//!
//! No trailer, no footer, no attribution, no mention of slidx. A commit message
//! is the author's record of their own talk, and a tool that signed its name in
//! somebody's history would be taking credit for their work.

use crate::model::{Deck, Slide};

/// Past this many bullets the body stops and says how many are left. A commit
/// message listing forty slides is a message nobody reads.
const BULLET_BUDGET: usize = 12;

/// Subject lines are read in `git log --oneline`, at whatever width that is.
const SUBJECT_BUDGET: usize = 72;

/// One slide's title, cut short so a subject line survives it.
const TITLE_BUDGET: usize = 32;

/// A pairing between the deck at HEAD and the deck on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pair {
    before: usize,
    after: usize,
}

/// What one save is about.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    /// Nothing to compare against: the deck's first commit.
    pub first: bool,
    pub slides: usize,
    pub added: Vec<String>,
    pub dropped: Vec<String>,
    /// Title, and where it went — one-based, the way a speaker counts.
    pub moved: Vec<(String, usize, usize)>,
    pub retitled: Vec<(String, String)>,
    /// Body changed, title did not.
    pub revised: Vec<String>,
    pub noted: Vec<String>,
    /// Title, and the budget before and after.
    pub retimed: Vec<(String, Option<u32>, Option<u32>)>,
    /// Deck-level frontmatter: what changed, and what it is now.
    pub deck: Vec<(&'static str, String)>,
}

impl Summary {
    /// Compares the deck at HEAD with the deck on disk.
    pub fn of(before: &Deck, after: &Deck) -> Self {
        let pairs = pair(&before.slides, &after.slides);
        let mut summary = Self { slides: after.slides.len(), ..Self::default() };

        for (index, slide) in after.slides.iter().enumerate() {
            if !pairs.iter().any(|pair| pair.after == index) {
                summary.added.push(title(slide));
            }
        }

        for (index, slide) in before.slides.iter().enumerate() {
            if !pairs.iter().any(|pair| pair.before == index) {
                summary.dropped.push(title(slide));
            }
        }

        for pair in &pairs {
            let (was, now) = (&before.slides[pair.before], &after.slides[pair.after]);

            if moved_in(&pairs, pair) {
                summary.moved.push((title(now), pair.before + 1, pair.after + 1));
            }

            match (was.title.as_deref(), now.title.as_deref()) {
                (Some(old), Some(new)) if old != new => {
                    summary.retitled.push((clipped(old), clipped(new)))
                }
                _ if was.content.trim() != now.content.trim() => summary.revised.push(title(now)),
                _ => {}
            }

            if was.notes_text() != now.notes_text() {
                summary.noted.push(title(now));
            }

            if was.budget_seconds != now.budget_seconds {
                summary.retimed.push((title(now), was.budget_seconds, now.budget_seconds));
            }
        }

        summary.deck = deck_fields(before, after);
        summary
    }

    /// The deck's first commit, where there is nothing to compare against.
    pub fn first(after: &Deck) -> Self {
        Self { first: true, slides: after.slides.len(), ..Self::default() }
    }

    pub fn is_empty(&self) -> bool {
        !self.first
            && self.added.is_empty()
            && self.dropped.is_empty()
            && self.moved.is_empty()
            && self.retitled.is_empty()
            && self.revised.is_empty()
            && self.noted.is_empty()
            && self.retimed.is_empty()
            && self.deck.is_empty()
    }

    /// The commit message: a subject, a blank line, and what happened.
    pub fn message(&self) -> String {
        let subject = self.subject();
        let body = self.body();

        if body.is_empty() {
            return format!("{subject}\n");
        }

        format!(
            "{subject}\n\n{}",
            body.iter().map(|line| format!("- {line}\n")).collect::<String>()
        )
    }

    /// What goes under the subject, one change per line.
    ///
    /// Empty when there is only one change, because that change is already the
    /// whole subject and repeating it underneath would be a body that says
    /// nothing the first line did not. Past a dozen it stops and counts the
    /// rest: a message listing forty slides is a message nobody reads.
    ///
    /// Public, and the reason the rule lives here rather than in `message`: the
    /// editor's history panel shows a subject and this list, and a panel that
    /// decided for itself when to repeat the subject would disagree with the
    /// commit message about what one change looks like.
    pub fn body(&self) -> Vec<String> {
        let bullets = self.bullets();
        if bullets.len() < 2 {
            return Vec::new();
        }

        let mut shown: Vec<String> = bullets.iter().take(BULLET_BUDGET).cloned().collect();
        let rest = bullets.len().saturating_sub(shown.len());
        if rest > 0 {
            shown.push(format!("and {rest} more"));
        }

        shown
    }

    /// The one line `git log --oneline` shows.
    ///
    /// The two most significant things that happened, joined. Two rather than
    /// all of them because a subject that lists five changes is a subject nobody
    /// finishes reading, and the body is right underneath.
    ///
    /// Public because a commit that already exists has a subject of its own, and
    /// the history panel shows this one beside it: what the author called the
    /// change, and what the deck says the change was.
    pub fn subject(&self) -> String {
        if self.first {
            return capitalised(&format!("add the deck, {}", counted(self.slides, "slide")));
        }

        let phrases = self.phrases();

        let joined = match phrases.len() {
            0 => "Save the deck".to_string(),
            1 => capitalised(&phrases[0]),
            _ => capitalised(&format!("{} and {}", phrases[0], phrases[1])),
        };

        if joined.chars().count() <= SUBJECT_BUDGET {
            return joined;
        }

        let alone = capitalised(&phrases[0]);
        if alone.chars().count() <= SUBJECT_BUDGET {
            return alone;
        }

        // Every phrase names a slide, and this one's title is long enough to
        // blow the line on its own. The body still has the detail.
        "Rework the deck".to_string()
    }

    /// What happened, most significant first.
    fn phrases(&self) -> Vec<String> {
        let mut phrases = Vec::new();

        if !self.added.is_empty() {
            phrases.push(some_of("add", &self.added, "slide"));
        }
        if !self.dropped.is_empty() {
            phrases.push(some_of("drop", &self.dropped, "slide"));
        }
        if !self.moved.is_empty() {
            phrases.push(match self.moved.len() {
                1 => format!("move {} to slide {}", quoted(&self.moved[0].0), self.moved[0].2),
                count => format!("reorder {}", counted(count, "slide")),
            });
        }
        if !self.retitled.is_empty() {
            phrases.push(match self.retitled.len() {
                1 => format!(
                    "retitle {} to {}",
                    quoted(&self.retitled[0].0),
                    quoted(&self.retitled[0].1)
                ),
                count => format!("retitle {}", counted(count, "slide")),
            });
        }
        if !self.retimed.is_empty() {
            phrases.push(some_of(
                "retime",
                &self.retimed.iter().map(|(title, _, _)| title.clone()).collect::<Vec<_>>(),
                "slide",
            ));
        }
        if !self.noted.is_empty() {
            phrases.push(match self.noted.len() {
                1 => format!("write notes on {}", quoted(&self.noted[0])),
                count => format!("write notes on {}", counted(count, "slide")),
            });
        }
        if !self.revised.is_empty() {
            phrases.push(some_of("revise", &self.revised, "slide"));
        }
        if !self.deck.is_empty() {
            phrases.push(deck_phrase(&self.deck));
        }

        phrases
    }

    /// One line per change, in the same order the subject prefers them.
    fn bullets(&self) -> Vec<String> {
        let mut lines = Vec::new();

        for title in &self.added {
            lines.push(format!("added {}", quoted(title)));
        }
        for title in &self.dropped {
            lines.push(format!("dropped {}", quoted(title)));
        }
        for (title, from, to) in &self.moved {
            lines.push(format!("moved {} from slide {from} to slide {to}", quoted(title)));
        }
        for (was, now) in &self.retitled {
            lines.push(format!("retitled {} to {}", quoted(was), quoted(now)));
        }
        for (title, was, now) in &self.retimed {
            lines.push(format!(
                "budget on {}: {} to {}",
                quoted(title),
                length(*was),
                length(*now)
            ));
        }
        for title in &self.noted {
            lines.push(format!("notes on {}", quoted(title)));
        }
        for title in &self.revised {
            lines.push(format!("revised {}", quoted(title)));
        }
        for (field, value) in &self.deck {
            lines.push(format!("{field}: {value}"));
        }

        lines
    }
}

/// Which slides of the two decks are the same slide.
fn pair(before: &[Slide], after: &[Slide]) -> Vec<Pair> {
    let mut pairs: Vec<Pair> = Vec::new();
    let mut taken_before = vec![false; before.len()];
    let mut taken_after = vec![false; after.len()];

    let claim = |pairs: &mut Vec<Pair>,
                 taken_before: &mut Vec<bool>,
                 taken_after: &mut Vec<bool>,
                 same: &dyn Fn(&Slide, &Slide) -> bool| {
        for (a, slide) in after.iter().enumerate() {
            if taken_after[a] {
                continue;
            }

            let found = before
                .iter()
                .enumerate()
                .find(|(b, candidate)| !taken_before[*b] && same(candidate, slide));

            if let Some((b, _)) = found {
                taken_before[b] = true;
                taken_after[a] = true;
                pairs.push(Pair { before: b, after: a });
            }
        }
    };

    // Strongest evidence first: a slide that is byte-identical is that slide,
    // wherever it has moved to.
    claim(&mut pairs, &mut taken_before, &mut taken_after, &|was, now| {
        was.title == now.title && was.content.trim() == now.content.trim()
    });
    claim(&mut pairs, &mut taken_before, &mut taken_after, &|was, now| {
        !was.id.is_empty() && was.id == now.id
    });

    // Nothing left to go on. The k-th unmatched slide before is the k-th
    // unmatched slide after far more often than it is not — that is a slide
    // that was edited and retitled at once.
    let leftover_before: Vec<usize> =
        (0..before.len()).filter(|index| !taken_before[*index]).collect();
    let leftover_after: Vec<usize> =
        (0..after.len()).filter(|index| !taken_after[*index]).collect();

    for (b, a) in leftover_before.into_iter().zip(leftover_after) {
        pairs.push(Pair { before: b, after: a });
    }

    pairs.sort_by_key(|pair| pair.after);
    pairs
}

/// True when a pair sits in a different place relative to the slides around it.
///
/// Relative, not absolute: adding a slide at the top pushes everything down by
/// one and moves nothing. What counts as a move is a slide that changed places
/// with another slide.
fn moved_in(pairs: &[Pair], pair: &Pair) -> bool {
    pairs
        .iter()
        .any(|other| (other.before < pair.before) != (other.after < pair.after) && other != pair)
}

/// The deck's own frontmatter, where it changed.
fn deck_fields(before: &Deck, after: &Deck) -> Vec<(&'static str, String)> {
    let mut fields = Vec::new();

    if before.meta.title != after.meta.title {
        fields.push(("title", after.meta.display_title().to_string()));
    }
    if before.meta.duration_seconds != after.meta.duration_seconds {
        fields.push(("slot", length(after.meta.duration_seconds)));
    }
    if before.meta.theme != after.meta.theme {
        fields.push(("theme", after.meta.theme.clone().unwrap_or_else(|| "default".into())));
    }
    if before.meta.talk.event != after.meta.talk.event {
        fields.push(("event", after.meta.talk.event.clone().unwrap_or_else(|| "none".into())));
    }
    if before.meta.talk.date != after.meta.talk.date {
        fields.push(("date", after.meta.talk.date.clone().unwrap_or_else(|| "none".into())));
    }

    fields
}

fn deck_phrase(fields: &[(&'static str, String)]) -> String {
    match fields {
        [(field, value)] => match *field {
            "title" => format!("retitle the deck to {}", quoted(value)),
            "slot" => format!("set the slot to {value}"),
            "theme" => format!("switch to the {value} theme"),
            other => format!("set the {other} to {value}"),
        },
        _ => format!(
            "update the deck's {}",
            fields.iter().map(|(field, _)| *field).collect::<Vec<_>>().join(" and ")
        ),
    }
}

/// `add "The fix"`, or `add 3 slides` once naming them would run long.
fn some_of(verb: &str, titles: &[String], noun: &str) -> String {
    match titles {
        [one] => format!("{verb} {}", quoted(one)),
        [one, two] => format!("{verb} {} and {}", quoted(one), quoted(two)),
        many => format!("{verb} {}", counted(many.len(), noun)),
    }
}

fn counted(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// A slide's title, or what to call one that has none.
fn title(slide: &Slide) -> String {
    clipped(&slide.display_title())
}

fn clipped(title: &str) -> String {
    if title.chars().count() <= TITLE_BUDGET {
        return title.to_string();
    }

    let kept: String = title.chars().take(TITLE_BUDGET - 1).collect();
    format!("{}…", kept.trim_end())
}

/// Quoted, because a title is somebody's words and a message reads better when
/// it is clear where they start.
fn quoted(text: &str) -> String {
    format!("\"{text}\"")
}

/// A budget or a slot, in the spelling frontmatter uses.
fn length(seconds: Option<u32>) -> String {
    match seconds {
        None => "none".to_string(),
        Some(seconds) if seconds % 60 == 0 => format!("{}m", seconds / 60),
        Some(seconds) if seconds < 60 => format!("{seconds}s"),
        Some(seconds) => format!("{}m{}s", seconds / 60, seconds % 60),
    }
}

fn capitalised(text: &str) -> String {
    let mut characters = text.chars();

    match characters.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), characters.as_str()),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{parse_deck, DeckParseOptions};

    fn deck(source: &str) -> Deck {
        parse_deck(source, &DeckParseOptions::default())
    }

    fn summary(before: &str, after: &str) -> Summary {
        Summary::of(&deck(before), &deck(after))
    }

    fn message(before: &str, after: &str) -> String {
        summary(before, after).message()
    }

    const DECK: &str = "---\ntitle: Making decks fast\nduration: 20m\n---\n\n# Making decks fast\n\n---\n\n# What goes wrong\n\nthe wifi\n\n---\n\n# The fix\n";

    #[test]
    fn a_slide_added_is_reported_as_a_slide_and_not_as_thirty_four_lines() {
        // The whole reason this command is not `git commit -a`. git can say
        // `+34 -6`; a parser can say what was added.
        let after = format!("{DECK}\n---\n\n# What it cost\n");
        let text = message(DECK, &after);

        assert_eq!(text.lines().next(), Some("Add \"What it cost\""));
    }

    #[test]
    fn two_slides_added_are_named_and_three_are_counted() {
        let two = format!("{DECK}\n---\n\n# One more\n\n---\n\n# And another\n");
        assert!(message(DECK, &two).starts_with("Add \"One more\" and \"And another\""));

        let three = format!("{two}\n---\n\n# A third\n");
        assert!(message(DECK, &three).starts_with("Add 3 slides"));
    }

    #[test]
    fn a_dropped_slide_is_named_by_the_title_it_had() {
        // The slide is gone from the file, so the only place its name survives
        // is the message about removing it.
        let after = "---\ntitle: Making decks fast\nduration: 20m\n---\n\n# Making decks fast\n\n---\n\n# The fix\n";
        let summary = summary(DECK, after);

        assert_eq!(summary.dropped, ["What goes wrong"]);
        assert!(summary.message().starts_with("Drop \"What goes wrong\""), "{}", summary.message());
    }

    #[test]
    fn a_reorder_reads_as_a_reorder_rather_than_as_the_whole_deck_being_rewritten() {
        // The pairing's first pass exists for this: two identical slides that
        // swapped places are the same two slides.
        let after = "---\ntitle: Making decks fast\nduration: 20m\n---\n\n# Making decks fast\n\n---\n\n# The fix\n\n---\n\n# What goes wrong\n\nthe wifi\n";
        let summary = summary(DECK, after);

        assert!(summary.added.is_empty(), "{summary:?}");
        assert!(summary.dropped.is_empty(), "{summary:?}");
        assert!(!summary.moved.is_empty(), "{summary:?}");
        assert!(summary.message().contains("slide"), "{}", summary.message());
    }

    #[test]
    fn adding_a_slide_at_the_top_moves_nothing() {
        // Everything below it shifts by one, and none of it changed places with
        // anything. A message saying four slides moved would be noise.
        let after = "---\ntitle: Making decks fast\nduration: 20m\n---\n\n# A new opening\n\n---\n\n# Making decks fast\n\n---\n\n# What goes wrong\n\nthe wifi\n\n---\n\n# The fix\n";
        let summary = summary(DECK, after);

        assert_eq!(summary.added, ["A new opening"]);
        assert!(summary.moved.is_empty(), "{:?}", summary.moved);
    }

    #[test]
    fn an_edited_slide_is_revised_rather_than_added_and_dropped() {
        // Its id is the slug of a title that did not change, which is the
        // second pass's whole job.
        let after = DECK.replace("the wifi", "the venue wifi, and the fonts");
        let summary = summary(DECK, &after);

        assert_eq!(summary.revised, ["What goes wrong"]);
        assert!(summary.added.is_empty(), "{summary:?}");
    }

    #[test]
    fn a_slide_edited_and_retitled_at_once_is_still_the_same_slide() {
        // Nothing links the two but their position among what is left, which is
        // the third pass. Reporting an add and a drop would be wrong about what
        // happened.
        let after = DECK.replace(
            "# What goes wrong\n\nthe wifi",
            "# What actually goes wrong\n\nthe venue wifi",
        );
        let summary = summary(DECK, &after);

        assert_eq!(
            summary.retitled,
            [("What goes wrong".to_string(), "What actually goes wrong".to_string())]
        );
        assert!(summary.added.is_empty(), "{summary:?}");
        assert!(summary.dropped.is_empty(), "{summary:?}");
    }

    #[test]
    fn notes_written_over_a_slide_are_a_change_worth_naming() {
        // Nobody sees them and they are most of the work. git shows them as an
        // HTML comment; this says the speaker wrote what they are going to say.
        let after =
            DECK.replace("# The fix", "# The fix\n\n<!-- notes:\nOpen with the outcome.\n-->");
        let summary = summary(DECK, &after);

        assert_eq!(summary.noted, ["The fix"]);
        assert!(
            summary.message().starts_with("Write notes on \"The fix\""),
            "{}",
            summary.message()
        );
    }

    #[test]
    fn writing_notes_is_not_also_reported_as_revising_the_slide() {
        // They come out of the body during parsing, so a slide whose notes
        // changed must not read as two changes.
        let after =
            DECK.replace("# The fix", "# The fix\n\n<!-- notes:\nOpen with the outcome.\n-->");

        assert!(summary(DECK, &after).revised.is_empty());
    }

    #[test]
    fn a_budget_change_is_reported_in_the_spelling_the_frontmatter_uses() {
        // Two of them, so the message has a body: a single change is the whole
        // subject and repeating it underneath would say nothing new.
        let before = "---\ntitle: A talk\n---\n\n# One\n\n---\nbudget: 90s\n---\n\n# Two\n\n---\nbudget: 30s\n---\n\n# Three\n";
        let after = "---\ntitle: A talk\n---\n\n# One\n\n---\nbudget: 2m\n---\n\n# Two\n\n---\nbudget: 45s\n---\n\n# Three\n";
        let summary = summary(before, after);

        assert_eq!(
            summary.retimed,
            [("Two".to_string(), Some(90), Some(120)), ("Three".to_string(), Some(30), Some(45))]
        );
        assert!(summary.message().contains("1m30s to 2m"), "{}", summary.message());
        assert!(summary.message().contains("30s to 45s"), "{}", summary.message());
    }

    #[test]
    fn a_changed_slot_is_a_change_to_the_deck_rather_than_to_a_slide() {
        let after = DECK.replace("duration: 20m", "duration: 25m");
        let summary = summary(DECK, &after);

        assert_eq!(summary.deck, [("slot", "25m".to_string())]);
        assert_eq!(summary.message().lines().next(), Some("Set the slot to 25m"));
    }

    #[test]
    fn two_kinds_of_change_are_joined_and_the_rest_go_underneath() {
        let after =
            format!("{}\n---\n\n# What it cost\n", DECK.replace("duration: 20m", "duration: 25m"));
        let text = message(DECK, &after);
        let subject = text.lines().next().expect("a subject");

        assert!(subject.starts_with("Add \"What it cost\" and"), "{text}");
        assert!(text.contains("\n\n- "), "{text}");
    }

    #[test]
    fn a_first_commit_says_what_the_deck_is_rather_than_what_changed() {
        // There is nothing to compare against, and "34 slides added" would be a
        // strange way to describe a deck arriving.
        let summary = Summary::first(&deck(DECK));

        assert_eq!(summary.message().trim(), "Add the deck, 3 slides");
        assert!(!summary.is_empty());
    }

    #[test]
    fn a_deck_that_did_not_change_has_nothing_to_say() {
        // Which is what stops `slidx save` making an empty commit.
        assert!(summary(DECK, DECK).is_empty());
    }

    #[test]
    fn a_subject_line_stays_inside_seventy_two_columns() {
        // Read in `git log --oneline`, where a long one is truncated by
        // whatever is showing it.
        let long = "# A slide with a title that goes on considerably longer than anybody would put on a slide";
        let after = format!("{DECK}\n---\n\n{long}\n");
        let text = message(DECK, &after);

        for line in text.lines().take(1) {
            assert!(
                line.chars().count() <= SUBJECT_BUDGET,
                "{} columns: {line}",
                line.chars().count()
            );
        }
    }

    #[test]
    fn a_message_with_one_change_in_it_has_no_body_repeating_the_subject() {
        let after = format!("{DECK}\n---\n\n# What it cost\n");

        assert_eq!(message(DECK, &after).trim(), "Add \"What it cost\"");
    }

    #[test]
    fn a_message_stops_listing_after_a_dozen_bullets_and_says_how_many_are_left() {
        let mut after = DECK.to_string();
        for number in 0..20 {
            after.push_str(&format!("\n---\n\n# Slide {number}\n"));
        }

        let text = message(DECK, &after);

        assert!(text.contains("- and 8 more"), "{text}");
        assert!(text.lines().filter(|line| line.starts_with("- ")).count() <= BULLET_BUDGET + 1);
    }

    #[test]
    fn nothing_is_appended_to_a_message_ever() {
        // Not a trailer, not a footer, not an attribution, and no mention of
        // slidx. A commit message is the author's record of their own talk, and
        // a tool that signed its name in somebody's history would be taking
        // credit for their work.
        let after = format!("{DECK}\n---\n\n# What it cost\n");
        let text = message(DECK, &after);

        for forbidden in ["Co-authored-by", "Co-Authored-By", "Signed-off-by", "slidx", "🤖"] {
            assert!(!text.contains(forbidden), "{forbidden} appeared in:\n{text}");
        }
    }

    #[test]
    fn a_message_ends_with_exactly_one_newline() {
        // git strips trailing blank lines anyway; producing them would just
        // make the message differ from what was printed by --dry-run.
        let after = format!("{DECK}\n---\n\n# What it cost\n");
        let text = message(DECK, &after);

        assert!(text.ends_with('\n'));
        assert!(!text.ends_with("\n\n"));
    }

    #[test]
    fn a_slide_with_no_heading_is_named_by_its_number_rather_than_left_blank() {
        let after = format!("{DECK}\n---\n\nA slide that is a quotation and has no heading.\n");
        let summary = summary(DECK, &after);

        assert_eq!(summary.added, ["Slide 4"]);
    }

    #[test]
    fn a_length_reads_the_way_it_is_written_in_frontmatter() {
        assert_eq!(length(Some(1200)), "20m");
        assert_eq!(length(Some(90)), "1m30s");
        assert_eq!(length(Some(45)), "45s");
        assert_eq!(length(None), "none");
    }
}
