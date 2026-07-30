//! Where a deck's bytes live: one source to the parser, a directory to the
//! author.
//!
//! [`slidx_edit`] splices the joined source, and something has to say which file
//! each spliced byte was in. That is this module, and the rule it keeps is that
//! **nothing here decides what Markdown looks like**. A file's new bytes are
//! always a slice of the source the operation returned, wrapped in the
//! whitespace that file already had at its edges. slidx has exactly one writer
//! of Markdown and it is `slidx_edit`; a second one here would end the promise
//! that a generated diff is reviewable.
//!
//! # What a file owns
//!
//! A file holds whole slides — the deck is joined *with* the separator, so a
//! file boundary is always a slide boundary. A deck where that is not true (an
//! unclosed fence swallows the separator between two files) is refused rather
//! than half-written.
//!
//! The bytes before the first slide are the deck's own frontmatter, which
//! belongs to no slide. They follow whichever slide is first, so deleting the
//! opening slide does not take the deck's title with it.
//!
//! # Why this is stated in Rust as well as in TypeScript
//!
//! `packages/vite-plugin/src/files.ts` says the same thing for the visual
//! editor, whose writes come from a browser through the wasm boundary. This says
//! it for `slidx mcp`, whose writes happen in this process. Neither can call the
//! other, and the alternative — an agent writing whole files — is the failure
//! the operation set exists to prevent.
//!
//! What is not tolerable is the two disagreeing, so the rule is written down
//! rather than inferred, and it is three sentences long:
//!
//! 1. Files sort by name — which is why the convention is `0001.md`.
//! 2. Each file is trimmed and joined with the separator on its own line, with a
//!    blank line under it.
//! 3. A file that already opens with a separator brings its own.

use std::path::{Path, PathBuf};

use slidx_core::ByteSpan;
use slidx_edit::SlideSpans;

/// A slide file, as found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckFile {
    pub path: PathBuf,
    /// What to call it in a message. The name, not the whole path.
    pub label: String,
    pub source: String,
}

impl DeckFile {
    pub fn new(path: impl Into<PathBuf>, source: impl Into<String>) -> Self {
        let path = path.into();
        let label = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());

        Self { path, label, source: source.into() }
    }
}

/// What to do to one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWrite {
    pub path: PathBuf,
    pub label: String,
    /// The file's new bytes, or `None` when no slide is left in it.
    pub source: Option<String>,
}

/// A deck source, with its slides located in it.
#[derive(Debug, Clone)]
pub struct Located {
    pub source: String,
    pub slides: Vec<SlideSpans>,
}

/// The deck as the parser reads it, and where each file sits in it.
#[derive(Debug, Clone)]
pub struct Joined {
    pub source: String,
    /// One span per file, in the order the files were given.
    pub spans: Vec<ByteSpan>,
}

/// Joins slide files into the source the parser reads.
///
/// The join has to be the exact inverse of the cut, or a byte offset stops
/// meaning anything the moment the files are read back. Two rules make it one:
///
/// **A blank line under the separator.** A separator followed immediately by
/// lines that happen to parse as YAML — `## Heading` is a comment and
/// `<!-- notes: x -->` is a key — is how a slide declares its own frontmatter,
/// so joining tightly swallows the next file's first slide into the one before
/// it. That is not a hypothetical: a three-file deck whose middle file opens
/// with a notes comment loses a slide.
///
/// **A file that already opens with a separator brings its own.** That line is
/// the opening delimiter of the slide's frontmatter block *and* the break
/// between the two slides; writing another above it would leave an empty slide
/// between them.
///
/// A file with nothing in it contributes no separator, so a file this session
/// emptied does not add a blank slide. It keeps its place in the list, which is
/// what lets an undo put its slides back where they were.
pub fn join(files: &[DeckFile], separator: &str) -> Joined {
    let mut source = String::new();
    let mut spans = Vec::with_capacity(files.len());

    for file in files {
        let trimmed = file.source.trim();

        if !trimmed.is_empty() && !source.is_empty() {
            if opens_with_separator(trimmed, separator) {
                source.push_str("\n\n");
            } else {
                source.push_str(&format!("\n\n{separator}\n\n"));
            }
        }

        let start = source.len();
        source.push_str(trimmed);
        spans.push(ByteSpan::new(start, source.len()));
    }

    Joined { source, spans }
}

/// True when a file's first line is the deck separator and nothing else.
fn opens_with_separator(source: &str, separator: &str) -> bool {
    let first = source.lines().next().unwrap_or_default().trim_end();

    first.len() - first.trim_start().len() <= 3 && first.trim() == separator
}

/// Which files an edit changed, and what they now say.
///
/// A file whose bytes are the same as before is not in the result at all — not
/// "written with identical content". The difference is a modification time that
/// never moves and a watcher that never fires.
///
/// Refuses rather than half-writes when a slide crosses a file boundary, because
/// there is no honest answer about which file its bytes belong to.
pub fn plan_writes(
    files: &[DeckFile],
    separator: &str,
    before: &Located,
    after: &Located,
) -> Result<Vec<FileWrite>, String> {
    let spans = join(files, separator).spans;

    let owner: Vec<usize> = before
        .slides
        .iter()
        .map(|slide| owner_of(files, &spans, slide.content))
        .collect::<Result<_, _>>()?;

    let assignment =
        assign(&owner, &opening(&spans, &before.slides), &texts(before), &texts(after));

    let mut writes = Vec::new();

    for (index, file) in files.iter().enumerate() {
        let mine: Vec<usize> = assignment
            .iter()
            .enumerate()
            .filter(|(_, owned)| **owned == index)
            .map(|(slide, _)| slide)
            .collect();

        let source =
            if mine.is_empty() { None } else { Some(rewrap(&file.source, &cut(after, &mine))) };

        // Nothing left, and nothing there: a file this session already emptied.
        if source.is_none() && file.source.is_empty() {
            continue;
        }
        if source.as_deref() == Some(file.source.as_str()) {
            continue;
        }

        writes.push(FileWrite { path: file.path.clone(), label: file.label.clone(), source });
    }

    Ok(writes)
}

/// The file a slide's bytes are in.
fn owner_of(files: &[DeckFile], spans: &[ByteSpan], slide: ByteSpan) -> Result<usize, String> {
    let found = spans.iter().position(|span| {
        span.start <= slide.start && slide.end <= span.end && span.end > span.start
    });

    if let Some(index) = found {
        return Ok(index);
    }

    let crossed = spans.iter().position(|span| span.end > slide.start).unwrap_or(0);
    let label = files.get(crossed).map(|file| file.label.as_str()).unwrap_or("a slide file");

    Err(format!(
        "A slide runs past the end of {label}. Each file has to hold whole slides — an \
         unclosed code fence is the usual cause, because it swallows the separator between \
         one file and the next."
    ))
}

/// For each file, the index of the first slide that is not behind it.
///
/// This is what a slide with no predecessor to inherit from lands on: an
/// inserted slide joins the file it pushed down, and a slide restored by an undo
/// finds the file it was removed from still holding its place.
fn opening(spans: &[ByteSpan], slides: &[SlideSpans]) -> Vec<usize> {
    spans
        .iter()
        .map(|span| slides.iter().filter(|slide| slide.content.end <= span.start).count())
        .collect()
}

/// Which file each slide of the edited deck belongs in.
///
/// Derived from what changed rather than from which operation was run, so a new
/// operation in `slidx_edit` needs no case here. The unchanged slides at each end
/// keep their file; the run between them is matched up one for one for as far as
/// it goes, which is what makes a reorder move bytes between files instead of
/// piling them into one.
fn assign(owner: &[usize], opens: &[usize], before: &[&str], after: &[&str]) -> Vec<usize> {
    let prefix = shared(before, after);
    let suffix = shared_from_end(before, after, prefix);
    let old_run_end = before.len() - suffix;
    let new_run_end = after.len() - suffix;

    (0..after.len())
        .map(|index| {
            if index < prefix {
                return owner.get(index).copied().unwrap_or_default();
            }
            if index >= new_run_end {
                let mirrored = index + before.len() - after.len();
                return owner.get(mirrored).copied().unwrap_or_default();
            }
            if old_run_end > prefix {
                return owner.get(index.min(old_run_end - 1)).copied().unwrap_or_default();
            }

            // Nothing was replaced, so these slides are new. They go where the
            // slide they displaced starts, or in the last file when there is
            // nothing after them to displace.
            match opens.iter().position(|from| *from >= prefix) {
                Some(landing) => landing,
                None => owner.len().saturating_sub(1),
            }
        })
        .collect()
}

fn shared(before: &[&str], after: &[&str]) -> usize {
    (0..before.len().min(after.len())).take_while(|index| before[*index] == after[*index]).count()
}

fn shared_from_end(before: &[&str], after: &[&str], prefix: usize) -> usize {
    let room = before.len().min(after.len()).saturating_sub(prefix);

    (0..room)
        .take_while(|count| before[before.len() - 1 - count] == after[after.len() - 1 - count])
        .count()
}

fn texts(located: &Located) -> Vec<&str> {
    located.slides.iter().map(|slide| slide.content.slice(&located.source)).collect()
}

/// The bytes of a run of slides, taken whole from the edited source.
///
/// A run that opens the deck starts at byte zero rather than at its first slide,
/// because the deck's own frontmatter sits above every slide and belongs to no
/// one of them.
fn cut(after: &Located, slides: &[usize]) -> String {
    let first = slides.first().copied().unwrap_or_default();
    let last = slides.last().copied().unwrap_or_default();

    let start = if first == 0 {
        0
    } else {
        after.slides.get(first).map(|slide| slide.content.start).unwrap_or_default()
    };
    let end = after.slides.get(last).map(|slide| slide.content.end).unwrap_or(start);

    after.source.get(start..end).unwrap_or_default().to_string()
}

/// New content, in the whitespace the file already had around its own.
///
/// A file that loses its final newline reads as a whole-file change in every
/// review tool, which is the opposite of what a splice is for. A file that had
/// nothing in it gets one, because every text file ends with a line.
fn rewrap(original: &str, content: &str) -> String {
    if original.is_empty() {
        return format!("{content}\n");
    }

    let lead = original.len() - original.trim_start().len();
    let trail = (original.len() - original.trim_end().len()).min(original.len() - lead);

    format!("{}{content}{}", &original[..lead], &original[original.len() - trail..])
}

/// Reads every file of a deck, in deck order.
///
/// A single-file deck is one file, which is why [`join`] and [`plan_writes`]
/// need no case for it.
///
/// A named file that is no longer there reads as **empty** rather than as a
/// failure, and that is what makes an undo able to put a deleted slide file back.
/// Removing a slide deletes the file it was alone in — an empty slide file would
/// otherwise join the deck as a blank slide — so the reverse edit arrives at a
/// directory that is missing it. Keeping the path in the list with nothing in it
/// is how the restored slide finds the place it came from. Any other read failure
/// is still a failure: a permission error must not look like an empty slide.
pub fn read_files(path: &Path, files: &[PathBuf]) -> Result<Vec<DeckFile>, String> {
    if files.is_empty() {
        return Ok(vec![DeckFile::new(path, read_or_empty(path)?)]);
    }

    files.iter().map(|file| Ok(DeckFile::new(file, read_or_empty(file)?))).collect()
}

fn read_or_empty(path: &Path) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(source) => Ok(source),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(format!("Could not read {}: {error}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::DeckParseOptions;

    const SEPARATOR: &str = "---";

    fn files(sources: &[&str]) -> Vec<DeckFile> {
        sources
            .iter()
            .enumerate()
            .map(|(index, source)| DeckFile::new(format!("/deck/{:04}.md", index + 1), *source))
            .collect()
    }

    fn locate(source: &str) -> Located {
        let options = DeckParseOptions { separator: SEPARATOR.into(), ..Default::default() };

        Located { source: source.to_string(), slides: slidx_edit::slide_spans(source, &options) }
    }

    #[test]
    fn slide_files_are_joined_with_a_blank_line_under_the_separator() {
        // The one the CLI's own reader gets wrong. A file whose first line
        // parses as YAML — a notes comment does — is swallowed into the slide
        // above it when the join is tight.
        let joined = join(&files(&["# One\n", "<!-- notes: say this -->\n\n# Two\n"]), SEPARATOR);

        assert_eq!(joined.source, "# One\n\n---\n\n<!-- notes: say this -->\n\n# Two");
        assert_eq!(locate(&joined.source).slides.len(), 2, "both slides survive the join");
    }

    #[test]
    fn a_file_that_already_opens_with_a_separator_brings_its_own() {
        // That line is both the break between the slides and the opening
        // delimiter of the second one's frontmatter. Writing another above it
        // leaves an empty slide between them.
        let joined = join(&files(&["# One\n", "---\nlayout: split\n---\n\n# Two\n"]), SEPARATOR);

        assert_eq!(joined.source, "# One\n\n---\nlayout: split\n---\n\n# Two");
        assert_eq!(locate(&joined.source).slides.len(), 2);
    }

    #[test]
    fn a_files_span_is_the_bytes_it_contributed() {
        let deck = files(&["# One\n", "# Two\n"]);
        let joined = join(&deck, SEPARATOR);

        assert_eq!(joined.spans[0].slice(&joined.source), "# One");
        assert_eq!(joined.spans[1].slice(&joined.source), "# Two");
    }

    #[test]
    fn an_empty_file_contributes_no_separator_and_keeps_its_place() {
        // A file this session emptied must not add a blank slide, and has to
        // still be in the list so an undo can put its slides back.
        let joined = join(&files(&["# One\n", "", "# Three\n"]), SEPARATOR);

        assert_eq!(joined.source, "# One\n\n---\n\n# Three");
        assert_eq!(joined.spans.len(), 3);
    }

    #[test]
    fn a_file_keeps_the_whitespace_it_had_around_its_own_content() {
        // A file that loses its final newline reads as a whole-file change in
        // every review tool, which is the opposite of what a splice is for.
        let deck = files(&["# One\n"]);
        let before = locate(&join(&deck, SEPARATOR).source);
        let after = locate("# Renamed");

        let writes = plan_writes(&deck, SEPARATOR, &before, &after).expect("a plan");
        assert_eq!(writes[0].source.as_deref(), Some("# Renamed\n"));
    }

    #[test]
    fn a_file_whose_bytes_did_not_change_is_not_written_at_all() {
        // Not "written with identical content". The difference is a
        // modification time that never moves.
        let deck = files(&["# One\n", "# Two\n"]);
        let joined = join(&deck, SEPARATOR);
        let before = locate(&joined.source);
        let after = locate(&joined.source.replace("# Two", "# Renamed"));

        let writes = plan_writes(&deck, SEPARATOR, &before, &after).expect("a plan");

        assert_eq!(writes.len(), 1, "{writes:?}");
        assert_eq!(writes[0].label, "0002.md");
    }

    #[test]
    fn the_decks_own_frontmatter_follows_whichever_slide_is_first() {
        // Deleting the opening slide must not take the deck's title with it. The
        // frontmatter belongs to no slide, so it goes wherever the first
        // surviving one ended up — here that is the second file, because the
        // first has nothing left in it.
        let deck = files(&["---\ntitle: A talk\n---\n\n# One\n", "# Two\n"]);
        let before = locate(&join(&deck, SEPARATOR).source);
        let after = locate("---\ntitle: A talk\n---\n\n# Two");

        let writes = plan_writes(&deck, SEPARATOR, &before, &after).expect("a plan");
        let surviving: Vec<&str> =
            writes.iter().filter_map(|write| write.source.as_deref()).collect();

        assert_eq!(surviving.len(), 1, "{writes:?}");
        assert!(surviving[0].starts_with("---\ntitle: A talk"), "{surviving:?}");
        assert!(surviving[0].contains("# Two"));
    }

    #[test]
    fn a_file_that_lost_its_last_slide_is_removed_rather_than_emptied() {
        // An empty slide file joins the deck as a blank slide, and the deck
        // would gain one every time an author deleted a file's last slide.
        let deck = files(&["# One\n", "# Two\n"]);
        let before = locate(&join(&deck, SEPARATOR).source);
        let after = locate("# One");

        let writes = plan_writes(&deck, SEPARATOR, &before, &after).expect("a plan");
        let second = writes.iter().find(|write| write.label == "0002.md").expect("the second file");

        assert_eq!(second.source, None);
    }

    #[test]
    fn an_inserted_slide_joins_the_file_it_pushed_down() {
        let deck = files(&["# One\n", "# Two\n"]);
        let before = locate(&join(&deck, SEPARATOR).source);
        let after = locate("# One\n\n---\n\n# Inserted\n\n---\n\n# Two");

        let writes = plan_writes(&deck, SEPARATOR, &before, &after).expect("a plan");
        let holding: Vec<&str> = writes
            .iter()
            .filter(|write| write.source.as_deref().is_some_and(|text| text.contains("Inserted")))
            .map(|write| write.label.as_str())
            .collect();

        assert_eq!(holding, ["0002.md"], "{writes:?}");
    }

    #[test]
    fn a_slide_crossing_a_file_boundary_is_refused_rather_than_half_written() {
        // An unclosed fence swallows the separator between two files, and then
        // there is no honest answer about which file a slide's bytes are in.
        let deck = files(&["```rust\n# One\n", "# Two\n"]);
        let joined = join(&deck, SEPARATOR);
        let before = locate(&joined.source);

        let refusal = plan_writes(&deck, SEPARATOR, &before, &before).expect_err("refused");
        assert!(refusal.contains("whole slides"), "{refusal}");
        assert!(refusal.contains("0001.md"), "{refusal}");
    }

    #[test]
    fn the_join_and_the_cut_are_inverses() {
        // The property everything else here depends on. If they are not, a byte
        // offset stops meaning anything the moment the files are read back.
        for sources in [
            vec!["# One\n"],
            vec!["# One\n", "# Two\n"],
            vec!["---\ntitle: T\n---\n\n# One\n", "# Two\n", "# Three\n"],
            vec!["# One\n", "---\nlayout: split\n---\n\n# Two\n"],
            vec!["# One\n\n- a\n- b\n", "<!-- notes: hi -->\n\n# Two\n"],
        ] {
            let deck = files(&sources);
            let located = locate(&join(&deck, SEPARATOR).source);

            let writes = plan_writes(&deck, SEPARATOR, &located, &located).expect("a plan");
            assert!(writes.is_empty(), "an unchanged deck rewrites nothing: {writes:?}");
        }
    }
}
