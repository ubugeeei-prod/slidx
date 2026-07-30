//! What changed, said in slides rather than in lines.
//!
//! This is the whole reason `slidx save` is not an alias for `git commit -a`.
//! git compares two files and can say `+34 -6`; slidx has a parser, so it can
//! say *two slides added, one dropped, the demo retimed*. That is the sentence
//! the author would have typed, and the one they will want to read in a year
//! when they are looking for the version of the talk that had the live demo in
//! it.
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
//! ## What this deliberately does not decide
//!
//! How to say any of it. That is [`super::message`], which is a question about
//! English and about the width of a `git log --oneline` line — a different
//! question, changing for different reasons, and the two were one file until
//! that was obvious.

use slidx_core::{Deck, Slide};

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

/// A budget or a slot, in the spelling frontmatter uses.
///
/// Beside the comparison rather than beside the wording, because a deck field
/// is recorded already rendered — `("slot", "20m")` — and this is what renders
/// it.
pub(super) fn length(seconds: Option<u32>) -> String {
    match seconds {
        None => "none".to_string(),
        Some(seconds) if seconds % 60 == 0 => format!("{}m", seconds / 60),
        Some(seconds) if seconds < 60 => format!("{seconds}s"),
        Some(seconds) => format!("{}m{}s", seconds / 60, seconds % 60),
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

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

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
    fn a_deck_that_did_not_change_has_nothing_to_say() {
        // Which is what stops `slidx save` making an empty commit.
        assert!(summary(DECK, DECK).is_empty());
    }

    #[test]
    fn a_slide_with_no_heading_is_named_by_its_number_rather_than_left_blank() {
        let after = format!("{DECK}\n---\n\nA slide that is a quotation and has no heading.\n");
        let summary = summary(DECK, &after);

        assert_eq!(summary.added, ["Slide 4"]);
    }
}
