//! The commit message a summary turns into.
//!
//! Split from the comparison because they answer different questions.
//! [`super::summary`] decides *what happened* — which slides are the same
//! slide, what moved, what was retimed. This decides *how to say it*, which is
//! a question about English and about the width of a line in
//! `git log --oneline`.
//!
//! ## Two budgets
//!
//! A subject carries the two most significant things that happened and nothing
//! else. Not all of them: a subject listing five changes is one nobody finishes
//! reading, and the body is directly underneath it. Past twelve bullets the
//! body stops and says how many are left, because a commit message listing
//! forty slides is a message nobody reads either.
//!
//! ## Nothing is appended
//!
//! No trailer, no footer, no attribution, and no mention of slidx anywhere in
//! it. A commit message is the author's record of their own talk, and a tool
//! that signed its name in somebody's history would be taking credit for their
//! work. There is a test that reads a message back out of a real repository and
//! looks for exactly that.

use super::summary::{length, Summary};

/// Past this many bullets the body stops and says how many are left.
const BULLET_BUDGET: usize = 12;

/// Subject lines are read in `git log --oneline`, at whatever width that is.
const SUBJECT_BUDGET: usize = 72;

impl Summary {
    /// The commit message: a subject, a blank line, and what happened.
    pub fn message(&self) -> String {
        let subject = self.subject();
        let bullets = self.bullets();

        if bullets.len() < 2 {
            // One change is already the whole subject. Repeating it underneath
            // would be a body that says nothing the first line did not.
            return format!("{subject}\n");
        }

        let shown: Vec<String> = bullets.iter().take(BULLET_BUDGET).cloned().collect();
        let rest = bullets.len().saturating_sub(shown.len());
        let more = if rest > 0 { format!("- and {rest} more\n") } else { String::new() };

        format!(
            "{subject}\n\n{}{more}",
            shown.iter().map(|line| format!("- {line}\n")).collect::<String>()
        )
    }

    /// The one line `git log --oneline` shows.
    ///
    /// The two most significant things that happened, joined. Two rather than
    /// all of them because a subject that lists five changes is a subject nobody
    /// finishes reading, and the body is right underneath.
    fn subject(&self) -> String {
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

/// Quoted, because a title is somebody's words and a message reads better when
/// it is clear where they start.
fn quoted(text: &str) -> String {
    format!("\"{text}\"")
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
    use crate::save::summary::Summary;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn deck(source: &str) -> slidx_core::Deck {
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
    fn two_slides_added_are_named_and_three_are_counted() {
        let two = format!("{DECK}\n---\n\n# One more\n\n---\n\n# And another\n");
        assert!(message(DECK, &two).starts_with("Add \"One more\" and \"And another\""));

        let three = format!("{two}\n---\n\n# A third\n");
        assert!(message(DECK, &three).starts_with("Add 3 slides"));
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
    fn a_length_reads_the_way_it_is_written_in_frontmatter() {
        assert_eq!(length(Some(1200)), "20m");
        assert_eq!(length(Some(90)), "1m30s");
        assert_eq!(length(Some(45)), "45s");
        assert_eq!(length(None), "none");
    }
}
