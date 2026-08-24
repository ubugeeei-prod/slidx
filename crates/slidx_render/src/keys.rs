//! The keyboard, as one table.
//!
//! A speaker drives a deck with their hands off the screen — from a clicker,
//! from the keyboard, in the dark. That makes the binding list something they
//! need to **see**, so it has to be data rather than a switch buried in a
//! handler.
//!
//! `packages/runtime/src/keymap.ts` said exactly that and was imported by
//! nobody, while three other places bound keys. This is the table it described,
//! moved to the side of the boundary that can both emit a handler and render a
//! list — `slidx_render` writes the inline script *and* the presenter page, and
//! TypeScript writes neither.
//!
//! # What reads it
//!
//! - [`table`] emits the object `navigation::script` matches an unstaged
//!   slide's keys against.
//! - [`rows`] renders the list on the presenter view, which is the speaker's
//!   own screen. A list of keys on the projector is a list the audience reads.
//! - `scripts/check-keys.mjs` holds `packages/runtime/src/navigate.ts` to the
//!   same names, because a staged slide's keys come from the runtime and two
//!   tables that can disagree is the shape of #255 and #257 — both of which
//!   cost a talk.
//!
//! # Why the description is here and not beside each binding
//!
//! Because it is the half nobody was maintaining. A key with no sentence is a
//! key that cannot be listed, and a list is the only reason to keep the table
//! as data at all.

/// One row: what it does, the keys that do it, and how to say so.
#[derive(Debug)]
pub struct Binding {
    pub command: &'static str,
    /// Alternatives, in the order a help panel should read them.
    pub keys: &'static [&'static str],
    pub description: &'static str,
}

/// Every key a slide answers.
///
/// `f` and `d` are here with the rest even though `gestures` and `demo_switch`
/// bind them, because a speaker reading this list does not care which module
/// emitted what. The list is the product; the modules are how it is delivered.
pub const BINDINGS: &[Binding] = &[
    Binding {
        command: "next",
        keys: &["ArrowRight", "ArrowDown", "PageDown", " ", "Enter"],
        description: "Next stop, then next slide",
    },
    Binding {
        command: "previous",
        keys: &["ArrowLeft", "ArrowUp", "PageUp", "Backspace"],
        description: "Back one stop, then back one slide",
    },
    Binding { command: "first", keys: &["Home"], description: "First slide" },
    Binding { command: "last", keys: &["End"], description: "Last slide" },
    Binding {
        command: "fullscreen",
        keys: &["f"],
        description: "Take the whole screen, and ask to keep it awake",
    },
    Binding {
        command: "toggleDemo",
        keys: &["d"],
        description: "Switch a live demo for its recording",
    },
];

/// The keys a slide moves on, as the object an inline handler indexes.
///
/// Only the two directions. `Home`, `End`, `f` and `d` are each one comparison
/// at the point they are handled, and putting them in a lookup that maps to a
/// string the caller then has to branch on would cost bytes on every slide to
/// save none.
pub fn table() -> String {
    let entries = BINDINGS
        .iter()
        .filter(|binding| binding.command == "next" || binding.command == "previous")
        .flat_map(|binding| {
            let rel = if binding.command == "next" { "next" } else { "prev" };
            binding.keys.iter().map(move |key| format!("{}: \"{rel}\"", quoted(key)))
        })
        .collect::<Vec<_>>();

    format!("{{ {} }}", entries.join(", "))
}

/// A key as an object member: bare where it can be, quoted where it must be.
///
/// `" "` and `"Enter"` are both valid identifiers to a person and only one is
/// to a parser. Quoting only what needs it keeps the emitted table the size it
/// was when it was written by hand.
fn quoted(key: &str) -> String {
    if key.chars().all(|character| character.is_ascii_alphanumeric()) && !key.is_empty() {
        key.to_string()
    } else {
        format!("\"{key}\"")
    }
}

/// The list, as markup for the presenter view.
///
/// A `<dl>` because that is what it is: a term and what it means. Alternatives
/// are joined rather than listed, since a speaker glancing at this wants "any
/// of these" and not four rows of the same sentence.
pub fn rows() -> String {
    BINDINGS
        .iter()
        .map(|binding| {
            let keys = binding
                .keys
                .iter()
                .map(|key| format!("<kbd>{}</kbd>", crate::shell::escape(label(key))))
                .collect::<Vec<_>>()
                .join(" ");

            format!(
                "      <div class=\"slidx-key\"><dt>{keys}</dt><dd>{}</dd></div>\n",
                crate::shell::escape(binding.description)
            )
        })
        .collect()
}

/// What a key is called on a keycap rather than in an event.
///
/// `" "` is the one that matters: a speaker looking for the space bar will not
/// find it printed as a space, and every deck tool that shows one writes the
/// word.
fn label(key: &str) -> &str {
    match key {
        " " => "Space",
        "ArrowRight" => "→",
        "ArrowLeft" => "←",
        "ArrowUp" => "↑",
        "ArrowDown" => "↓",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_emitted_table_is_the_one_that_was_written_by_hand() {
        // The bytes this replaced, exactly. A generated table that was larger
        // would be paid for on every slide of every deck.
        assert_eq!(
            table(),
            r#"{ ArrowRight: "next", ArrowDown: "next", PageDown: "next", " ": "next", Enter: "next", ArrowLeft: "prev", ArrowUp: "prev", PageUp: "prev", Backspace: "prev" }"#
        );
    }

    #[test]
    fn a_key_that_is_not_an_identifier_is_quoted_and_the_rest_are_not() {
        assert_eq!(quoted("Enter"), "Enter");
        assert_eq!(quoted(" "), "\" \"");
        assert_eq!(quoted("f"), "f");
    }

    #[test]
    fn the_space_bar_is_listed_as_a_word() {
        // Nobody looking for the space bar finds it printed as a space.
        assert!(rows().contains("<kbd>Space</kbd>"), "{}", rows());
    }

    #[test]
    fn every_binding_carries_a_sentence_long_enough_to_be_one() {
        // The half nobody was maintaining. A key with no description cannot be
        // listed, and being listable is the only reason this is data.
        for binding in BINDINGS {
            assert!(
                binding.description.len() > 8,
                "{} has no description worth showing",
                binding.command
            );
        }
    }

    #[test]
    fn the_keys_a_gesture_stands_in_for_are_in_the_list() {
        // `gestures` dispatches `ArrowRight`, and `demo_switch` binds `d`.
        // Both are bound elsewhere and both belong in the one list a speaker
        // reads, which is the whole reason this table is not `navigation`'s.
        let list = rows();

        assert!(list.contains("<kbd>→</kbd>"), "{list}");
        assert!(list.contains("<kbd>d</kbd>"), "{list}");
        assert!(list.contains("<kbd>f</kbd>"), "{list}");
    }
}
