//! Applying one operation to a deck on disk, and handing back its inverse.
//!
//! Every write `slidx mcp` makes comes through here, and every byte of it was
//! computed by [`slidx_edit`]. This module joins the files, hands the operation
//! over, and puts the result back in the files it came from. It never inspects
//! an operation and never composes Markdown — if an agent needs a change the
//! operation set cannot express, the answer is a new operation in Rust with
//! tests, the same rule the visual editor lives under.
//!
//! ## Why the inverse comes back with every write
//!
//! [`slidx_edit::Edit`] is already a value that can take itself back, so this is
//! nearly free — and no other editing surface an agent has can offer it. An
//! agent that has just made six changes and been told the seventh was wrong can
//! walk back exactly six, byte for byte, rather than trying to remember what the
//! file said. The inverse is measured against the *joined* source, which is why
//! [`Applied::inverse`] is stored rather than recomputed: the bytes it names stop
//! meaning anything once anything else has been spliced.
//!
//! ## An operation that names nothing is an answer, not a failure
//!
//! An agent builds operations from a deck it read a moment ago. Naming a slide
//! that has since been deleted is ordinary traffic, and [`slidx_edit`] returns
//! those as values for exactly that reason.

use std::fs;

use slidx_core::DeckParseOptions;
use slidx_edit::{Edit, EditOp};

use super::deck::{self, Located};
use super::workspace::Reading;

/// What one operation did.
#[derive(Debug, Clone)]
pub struct Applied {
    /// The edit that takes this one back, against the source after it.
    pub inverse: Edit,
    /// The files whose bytes changed. Empty when the deck already said this.
    pub changed: Vec<String>,
    /// The deck source after the operation.
    pub source: String,
    /// True when the operation asked for what the source already said.
    pub redundant: bool,
}

/// Applies one operation and writes the files it changed.
///
/// The splice is planned against the joined source, and the joined source is cut
/// back along the same spans the splice was measured in — see [`super::deck`].
/// Nothing else can be done here safely: a file's new bytes have to be a *slice*
/// of what the operation produced, or slidx has a second writer of Markdown.
pub fn apply(reading: &Reading, op: &EditOp) -> Result<Applied, String> {
    plan(reading, |source, options| {
        slidx_edit::plan(source, options, op).map_err(|refusal| refusal.to_string())
    })
}

/// Applies an edit off the undo stack.
///
/// Redo is undo of undo, so this serves both directions: the inverse it hands
/// back is the edit that does the change again.
pub fn revert(reading: &Reading, edit: &Edit) -> Result<Applied, String> {
    plan(reading, |_, _| Ok(edit.clone()))
}

/// Normalises the parts of a deck slidx owns.
///
/// The same path as an operation, because [`slidx_fmt::plan`] returns the same
/// invertible [`Edit`]. So formatting a deck is undoable exactly like anything
/// else here — which is not a coincidence, it is what having one representation
/// of a change buys.
pub fn format(reading: &Reading) -> Result<Applied, String> {
    plan(reading, |source, options| Ok(slidx_fmt::plan(source, options)))
}

fn plan(
    reading: &Reading,
    compute: impl Fn(&str, &DeckParseOptions) -> Result<Edit, String>,
) -> Result<Applied, String> {
    let files = deck::read_files(&reading.path, &reading.files)?;
    let options =
        DeckParseOptions { separator: reading.separator.clone(), ..DeckParseOptions::default() };

    // Re-joined here rather than reusing what was parsed for reading, because
    // the splice and the cut have to be measured in the same document. The two
    // agree by construction only if one function produced both.
    let before = locate(&deck::join(&files, &reading.separator).source, &options);
    let edit = compute(&before.source, &options)?;

    if edit.is_empty() {
        return Ok(Applied {
            inverse: Edit::default(),
            changed: Vec::new(),
            source: before.source,
            redundant: true,
        });
    }

    let after = locate(&edit.apply(&before.source), &options);
    let inverse = edit.invert(&before.source);
    let writes = deck::plan_writes(&files, &reading.separator, &before, &after)?;

    let mut changed = Vec::with_capacity(writes.len());
    for write in &writes {
        match &write.source {
            Some(source) => fs::write(&write.path, source)
                .map_err(|error| format!("Could not write {}: {error}", write.path.display()))?,
            // A file with nothing left in it is removed rather than emptied: an
            // empty slide file joins the deck as a blank slide, and the deck
            // would gain one every time a slide file's last slide was deleted.
            None => fs::remove_file(&write.path)
                .map_err(|error| format!("Could not remove {}: {error}", write.path.display()))?,
        }

        changed.push(write.label.clone());
    }

    Ok(Applied { inverse, changed, source: after.source, redundant: false })
}

fn locate(source: &str, options: &DeckParseOptions) -> Located {
    Located { source: source.to_string(), slides: slidx_edit::slide_spans(source, options) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::workspace::Workspace;
    use std::path::{Path, PathBuf};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-edit-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(path.join("slides")).expect("scratch");
            Self(path)
        }

        fn slide(&self, name: &str, body: &str) {
            fs::write(self.0.join("slides").join(name), body).expect("write");
        }

        fn read(&self, name: &str) -> String {
            fs::read_to_string(self.0.join("slides").join(name)).unwrap_or_default()
        }

        fn exists(&self, name: &str) -> bool {
            self.0.join("slides").join(name).exists()
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn reading(&self) -> Reading {
            Workspace::new(vec![self.path().to_path_buf()])
                .with_index(self.path().join("no-index.json"))
                .read_deck(&self.path().display().to_string(), None)
                .expect("a deck")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn an_operation_changes_exactly_the_file_it_named() {
        let scratch = Scratch::new("one-file");
        scratch.slide("0001.md", "# One\n");
        scratch.slide("0002.md", "# Two\n\n- a\n- b\n");

        let applied = apply(
            &scratch.reading(),
            &EditOp::SetHeading { slide: 1.into(), text: "Renamed".into() },
        )
        .expect("applied");

        assert_eq!(applied.changed, ["0002.md"]);
        assert_eq!(scratch.read("0001.md"), "# One\n", "byte for byte");
        assert_eq!(scratch.read("0002.md"), "# Renamed\n\n- a\n- b\n");
    }

    #[test]
    fn the_authors_own_spacing_and_bullets_survive_an_edit() {
        // The whole reason this goes through an operation. A serialiser would
        // regularise every one of these, invisibly on the slide and enormously
        // in the diff.
        let scratch = Scratch::new("spacing");
        scratch.slide(
            "0001.md",
            "#   Introduction\n\n\n*  first\n*  second\n\nA hand-wrapped\nparagraph.\n",
        );

        apply(&scratch.reading(), &EditOp::SetHeading { slide: 0.into(), text: "Opening".into() })
            .expect("applied");

        assert_eq!(
            scratch.read("0001.md"),
            "#   Introduction\n\n\n*  first\n*  second\n\nA hand-wrapped\nparagraph.\n"
                .replace("Introduction", "Opening")
        );
    }

    #[test]
    fn an_operation_that_asks_for_what_the_source_already_says_writes_nothing() {
        // Idempotence is a property of `slidx_edit`, and this is where it shows
        // up: a modification time that never moves.
        let scratch = Scratch::new("redundant");
        scratch.slide("0001.md", "# One\n");

        let applied =
            apply(&scratch.reading(), &EditOp::SetHeading { slide: 0.into(), text: "One".into() })
                .expect("applied");

        assert!(applied.redundant);
        assert!(applied.changed.is_empty());
        assert!(applied.inverse.is_empty(), "there is nothing to take back");
    }

    #[test]
    fn the_inverse_puts_the_deck_back_byte_for_byte() {
        let scratch = Scratch::new("inverse");
        scratch.slide("0001.md", "---\ntitle: A talk\n---\n\n#  One\n\n*  a\n");
        scratch.slide("0002.md", "# Two\n");
        let before = (scratch.read("0001.md"), scratch.read("0002.md"));

        let applied = apply(
            &scratch.reading(),
            &EditOp::SetBody { slide: 0.into(), body: "# Rewritten".into() },
        )
        .expect("applied");
        assert_ne!(scratch.read("0001.md"), before.0);

        revert(&scratch.reading(), &applied.inverse).expect("reverted");

        assert_eq!((scratch.read("0001.md"), scratch.read("0002.md")), before);
    }

    #[test]
    fn a_slide_removed_from_its_own_file_takes_the_file_with_it() {
        let scratch = Scratch::new("remove");
        scratch.slide("0001.md", "# One\n");
        scratch.slide("0002.md", "# Two\n");

        apply(&scratch.reading(), &EditOp::RemoveSlide { slide: 1.into() }).expect("applied");

        assert!(!scratch.exists("0002.md"), "an empty slide file would join the deck as a slide");
        assert_eq!(scratch.read("0001.md"), "# One\n");
    }

    #[test]
    fn an_operation_naming_a_slide_that_is_gone_is_an_answer_rather_than_a_panic() {
        // An agent builds operations from a deck it read a moment ago, so this
        // race is ordinary traffic.
        let scratch = Scratch::new("gone");
        scratch.slide("0001.md", "# One\n");

        let refusal = apply(&scratch.reading(), &EditOp::RemoveSlide { slide: 7.into() })
            .expect_err("no such");

        assert!(refusal.contains("no slide at index 7"), "{refusal}");
    }

    #[test]
    fn a_single_file_deck_is_spliced_in_place() {
        let scratch = Scratch::new("single");
        let file = scratch.path().join("talk.md");
        fs::write(&file, "# One\n\n---\n\n# Two\n").expect("write");

        let reading = Workspace::new(vec![scratch.path().to_path_buf()])
            .with_index(scratch.path().join("no-index.json"))
            .read_deck(&file.display().to_string(), None)
            .expect("a deck");

        apply(&reading, &EditOp::SetHeading { slide: 1.into(), text: "Second".into() })
            .expect("applied");

        assert_eq!(fs::read_to_string(&file).expect("read"), "# One\n\n---\n\n# Second\n");
    }

    #[test]
    fn a_japanese_deck_is_spliced_by_bytes_rather_than_characters() {
        // Three bytes per kanji. A range counted in characters would land in
        // the middle of one and produce invalid UTF-8.
        let scratch = Scratch::new("japanese");
        scratch.slide("0001.md", "# 導入\n\n速度が上がりました。\n");

        let applied = apply(
            &scratch.reading(),
            &EditOp::SetHeading { slide: 0.into(), text: "はじめに".into() },
        )
        .expect("applied");

        assert_eq!(scratch.read("0001.md"), "# はじめに\n\n速度が上がりました。\n");
        revert(&scratch.reading(), &applied.inverse).expect("reverted");
        assert_eq!(scratch.read("0001.md"), "# 導入\n\n速度が上がりました。\n");
    }
}
