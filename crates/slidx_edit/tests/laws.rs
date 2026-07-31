//! The four properties that make bidirectional editing worth claiming.
//!
//! Each one is checked over a corpus of decks crossed with every operation,
//! rather than on the example that happened to occur to whoever wrote the
//! feature. The corpus is deliberately awkward — CRLF files, decks with no
//! frontmatter, decks that are nothing but frontmatter, slides made of fenced
//! Markdown that contains its own separators — because those are the files an
//! editor meets in a real repository and the ones a splice can get wrong.
//!
//! 1. **Minimal diff.** An operation touches only the lines it names.
//! 2. **Round trip.** Parsing an edited source agrees with what the operation
//!    said it would do to the model.
//! 3. **Idempotence.** Setting something to what it already says is not an
//!    edit, and doing a set twice is the same as doing it once.
//! 4. **No panics.** An operation naming something that is not there is an
//!    error value, on every source in the corpus.

use slidx_core::{parse_deck, Attributes, ByteSpan, DeckParseOptions, StepAction};
use slidx_edit::{apply, plan, EditOp, MarkAttributes, SlideRef};
use slidx_theme::layout::BlockWidth;

/// Decks chosen so that every structural case appears at least once.
fn corpus() -> Vec<&'static str> {
    vec![
        // The ordinary shape.
        "---\ntitle: T\nduration: 20m\n---\n\n# One\n\nBody.\n\n---\n\n# Two\n\n- a\n- b\n",
        // No frontmatter at all.
        "# One\n\nBody.\n\n---\n\n# Two\n",
        // Frontmatter and nothing else.
        "---\ntitle: Only\n---\n",
        // A slide with its own block, which owns the separator above it.
        "# One\n\n---\nlayout: split\nbudget: 90s\n---\n\n# Two\n\n---\n\n# Three\n",
        // Deck frontmatter and a per-slide block, the pair that cannot both
        // sit at the top of a file.
        "---\ntitle: T\n---\n\n# One\n\n---\nlayout: cover\n---\n\n# Two\n",
        // One slide, so removing it has nowhere to fall back to.
        "# Only\n\nBody.\n",
        // Empty.
        "",
        // Windows line endings.
        "---\r\ntitle: T\r\n---\r\n\r\n# One\r\n\r\nBody.\r\n\r\n<!-- notes: said -->\r\n\r\n---\r\n\r\n# Two\r\n",
        // Separators inside a fence, which are not separators.
        "# Slides\n\n```md\n# a\n\n---\n\n# b\n```\n\n---\n\n# After\n",
        // Marks, notes, and steps together.
        "---\nsteps:\n  - reveal: \".a\"\n  - hide: \".b\"\n---\n\n# One\n\nA [word]{#hero .accent} here.\n\n<!-- notes: remember -->\n",
        // Slide-local visual state in the Markdown body.
        "<style data-slidx>\n:root {\n  --slidx-layout: aside;\n  --slidx-color-surface: oklch(20% 0.02 260);\n}\n</style>\n\n# Styled\n",
        // Staged with markers rather than a list.
        "# One\n\n- a <!-- step -->\n- b <!-- step -->\n",
        // Ragged blank lines the author chose.
        "# One\n\n\n\nBody.\n\n\n---\n\n\n# Two\n",
        // No trailing newline.
        "# One\n\n---\n\n# Two",
        // A heading with a closing run of hashes.
        "## Balanced ##\n\nBody.\n",
    ]
}

/// Every operation, aimed at whatever the given deck has.
fn operations(source: &str) -> Vec<EditOp> {
    let deck = parse_deck(source, &DeckParseOptions::default());
    let last = deck.slides.len() - 1;
    let attributes = MarkAttributes::default().with_key("k").with_class("accent");

    let mut ops = vec![
        EditOp::SetBody { slide: 0.into(), body: "# Replaced\n\nNew body.".into() },
        EditOp::SetHeading { slide: 0.into(), text: "Retitled".into() },
        EditOp::SetHeading { slide: last.into(), text: "Retitled".into() },
        EditOp::InsertSlide { at: 0, body: "# Inserted".into() },
        EditOp::InsertSlide { at: last, body: "# Inserted".into() },
        EditOp::InsertSlide { at: last + 1, body: "# Inserted".into() },
        EditOp::DuplicateSlide { slide: 0.into(), after: None },
        EditOp::DuplicateSlide { slide: last.into(), after: None },
        EditOp::RemoveSlide { slide: 0.into() },
        EditOp::RemoveSlide { slide: last.into() },
        EditOp::MoveSlide { slide: 0.into(), to: last },
        EditOp::MoveSlide { slide: last.into(), to: 0 },
        EditOp::SetField { slide: 0.into(), key: "theme".into(), value: "terminal".into() },
        EditOp::SetField { slide: last.into(), key: "budget".into(), value: "45s".into() },
        EditOp::SetStyle {
            slide: 0.into(),
            property: "layout".into(),
            value: Some("aside".into()),
        },
        EditOp::SetStyle {
            slide: last.into(),
            property: "color-surface".into(),
            value: Some("oklch(20% 0.02 260)".into()),
        },
        EditOp::AddStep { slide: 0.into(), at: None, action: StepAction::reveal(".added") },
        EditOp::AddStep { slide: 0.into(), at: Some(0), action: StepAction::reveal(".added") },
        EditOp::AdoptSteps { slide: 0.into() },
        EditOp::AdoptSteps { slide: last.into() },
        EditOp::SetNotes { slide: 0.into(), notes: "said out loud".into() },
        EditOp::SetNotes { slide: 0.into(), notes: String::new() },
        EditOp::SetNotes { slide: last.into(), notes: "said out loud".into() },
    ];

    for (index, located) in
        slidx_edit::slide_spans(source, &DeckParseOptions::default()).iter().enumerate()
    {
        // Typing, aimed at the words a caret could really be in: a block
        // retyped whole, and a caret at the front of one.
        for block in &located.blocks {
            ops.push(EditOp::SetText {
                slide: index.into(),
                range: block.span,
                text: "Typed over it.".into(),
            });
            ops.push(EditOp::SetText {
                slide: index.into(),
                range: ByteSpan::empty(block.span.start),
                text: "Typed ".into(),
            });

            for mark in &block.marks {
                // Inside a mark's words, which is the case that must leave the
                // `#key` a step points at exactly where it was.
                ops.push(EditOp::SetText {
                    slide: index.into(),
                    range: mark.words,
                    text: "retyped".into(),
                });
                // And across its edge, which is the case with a choice to make.
                ops.push(EditOp::SetText {
                    slide: index.into(),
                    range: ByteSpan::new(mark.words.start + 1, mark.span.end),
                    text: "X".into(),
                });
            }
        }
    }

    for (index, slide) in deck.slides.iter().enumerate() {
        if !slide.steps.actions.is_empty() {
            ops.push(EditOp::RemoveStep { slide: index.into(), index: 0 });
            ops.push(EditOp::SetStep {
                slide: index.into(),
                index: 0,
                action: StepAction::hide(".replaced"),
            });
        }
        // A move needs two positions to be about anything, so it joins the
        // corpus only where the slide has a list long enough to reorder.
        if slide.steps.actions.len() > 1 {
            let end = slide.steps.actions.len() - 1;
            ops.push(EditOp::MoveStep { slide: index.into(), from: 0, to: end });
            ops.push(EditOp::MoveStep { slide: index.into(), from: end, to: 0 });
        }
        if !slide.marks.is_empty() {
            ops.push(EditOp::SetMark {
                slide: index.into(),
                mark: 0.into(),
                attributes: attributes.clone(),
            });
            ops.push(EditOp::RemoveMark { slide: index.into(), mark: 0.into() });
        }
        if !slide.blocks.is_empty() {
            ops.push(EditOp::SetBlockAttributes {
                slide: index.into(),
                block: 0.into(),
                attributes: Attributes::default().with_key("placed").with_class("side"),
            });
            ops.push(EditOp::SetBlockAttributes {
                slide: index.into(),
                block: 0.into(),
                attributes: Attributes::default(),
            });
            ops.push(EditOp::MoveBlock {
                slide: index.into(),
                block: 0.into(),
                to: 0,
                region: Some("side".into()),
            });
            ops.push(EditOp::DuplicateBlock { slide: index.into(), block: 0.into() });
            ops.push(EditOp::DuplicateBlock {
                slide: index.into(),
                block: (slide.blocks.len() - 1).into(),
            });

            // Every share, including the default — which is the one written by
            // taking the property away rather than by writing it.
            for width in BlockWidth::ALL {
                ops.push(EditOp::SetBlockWidth {
                    slide: index.into(),
                    block: 0.into(),
                    width: *width,
                });
            }
        }
        // A move needs two blocks to be about anything.
        if slide.blocks.len() > 1 {
            let end = slide.blocks.len() - 1;
            ops.push(EditOp::MoveBlock {
                slide: index.into(),
                block: 0.into(),
                to: end,
                region: None,
            });
            ops.push(EditOp::MoveBlock {
                slide: index.into(),
                block: end.into(),
                to: 0,
                region: None,
            });
            ops.push(EditOp::MoveBlock {
                slide: index.into(),
                block: end.into(),
                to: 0,
                region: Some("right".into()),
            });
        }
    }

    ops
}

fn parse(source: &str) -> slidx_core::Deck {
    parse_deck(source, &DeckParseOptions::default())
}

fn edited(source: &str, op: &EditOp) -> String {
    apply(source, &DeckParseOptions::default(), op).expect("the corpus only aims at what is there")
}

/// The titles the author wrote.
///
/// A deck always parses into at least one slide so that something renders, so
/// a file of nothing but frontmatter reports a slide nobody wrote. Comparing
/// written titles keeps these properties about slides rather than about that
/// fallback.
fn written_titles(source: &str) -> Vec<String> {
    parse(source).slides.iter().filter_map(|slide| slide.title.clone()).collect()
}

// ------------------------------------------------------------- minimal diff

/// How many of the original lines an edit did not leave alone, measured from
/// the longest common prefix and suffix so a changed line count does not read
/// as a rewritten file.
fn touched(before: &str, after: &str) -> usize {
    let old: Vec<&str> = before.lines().collect();
    let new: Vec<&str> = after.lines().collect();

    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let room = old.len().min(new.len()) - prefix;
    let suffix =
        old.iter().rev().zip(new.iter().rev()).take(room).take_while(|(a, b)| a == b).count();

    old.len().saturating_sub(prefix + suffix)
}

#[test]
fn an_operation_touches_only_the_lines_it_names() {
    // Retitling the first slide of a hundred-slide deck is one line. The point
    // of the whole crate is that the other ninety-nine are not read, so the
    // number here is a hard ceiling rather than a budget.
    let mut source = "---\ntitle: T\n---\n".to_string();
    for index in 0..100 {
        source.push_str(&format!("\n# Slide {index}\n\nBody {index}.\n\n---\n"));
    }
    let source = source.trim_end_matches("\n---\n").to_string() + "\n";

    let one_line = [
        EditOp::SetHeading { slide: 0.into(), text: "Retitled".into() },
        EditOp::SetHeading { slide: 50.into(), text: "Retitled".into() },
        EditOp::SetField { slide: 0.into(), key: "title".into(), value: "Renamed".into() },
    ];

    for op in one_line {
        let result = edited(&source, &op);
        assert_eq!(touched(&source, &result), 1, "{op:?} rewrote more than the line it named");
    }
}

#[test]
fn a_line_the_operation_does_not_name_keeps_the_formatting_the_author_gave_it() {
    // Every line here is written in a way a serialiser would tidy up. None of
    // them may change, because none of them is what the operation names.
    let source = "---\ntitle:    Spaced Out\n---\n\n#    Loose Heading\n\n*  star bullet\n*  another\n\n\n\nA very long paragraph that a formatter would want to rewrap at eighty columns but must not.\n";
    let result = edited(
        source,
        &EditOp::SetField { slide: 0.into(), key: "theme".into(), value: "editorial".into() },
    );

    assert!(result.contains("title:    Spaced Out"));
    assert!(result.contains("#    Loose Heading"));
    assert!(result.contains("*  star bullet"));
    assert!(result.contains("\n\n\n\nA very long paragraph"));
}

#[test]
fn a_file_written_with_windows_line_endings_stays_that_way() {
    // An author on Windows has a CRLF file. Writing one LF line into it puts a
    // `^M` on every following line of the diff, which is the opposite of what
    // a splice is for.
    let source = "---\r\ntitle: T\r\n---\r\n\r\n# One\r\n\r\nBody.\r\n";

    let writes = [
        EditOp::InsertSlide { at: 1, body: "# Added".into() },
        EditOp::SetField { slide: 0.into(), key: "theme".into(), value: "terminal".into() },
        EditOp::SetStyle {
            slide: 0.into(),
            property: "layout".into(),
            value: Some("aside".into()),
        },
        EditOp::SetNotes { slide: 0.into(), notes: "spoken".into() },
        EditOp::AddStep { slide: 0.into(), at: None, action: StepAction::reveal(".a") },
    ];

    for op in writes {
        let result = edited(source, &op);
        assert!(
            !result.replace("\r\n", "").contains('\n'),
            "{op:?} left a bare newline in {result:?}"
        );
    }
}

// --------------------------------------------------------------- round trip

#[test]
fn what_the_source_says_after_an_edit_is_what_the_operation_asked_for() {
    // Parsing an edited file agrees with what the operation promised to do to
    // the model it was aimed at — for every slide of every deck in the corpus,
    // rather than for the one the feature was written against.
    for source in corpus() {
        for index in 0..parse(source).slides.len() {
            let retitled = EditOp::SetHeading { slide: index.into(), text: "Retitled".into() };
            assert_eq!(
                parse(&edited(source, &retitled)).slides[index].title.as_deref(),
                Some("Retitled"),
                "{retitled:?} on {source:?}"
            );

            let spoken = EditOp::SetNotes { slide: index.into(), notes: "spoken aloud".into() };
            assert_eq!(
                parse(&edited(source, &spoken)).slides[index].notes,
                vec!["spoken aloud"],
                "{spoken:?} on {source:?}"
            );

            let budgeted =
                EditOp::SetField { slide: index.into(), key: "budget".into(), value: "45s".into() };
            assert_eq!(
                parse(&edited(source, &budgeted)).slides[index].budget_seconds,
                Some(45),
                "{budgeted:?} on {source:?}"
            );

            let styled = EditOp::SetStyle {
                slide: index.into(),
                property: "layout".into(),
                value: Some("aside".into()),
            };
            assert_eq!(
                parse(&edited(source, &styled)).slides[index]
                    .style
                    .get("layout")
                    .map(String::as_str),
                Some("aside"),
                "{styled:?} on {source:?}"
            );

            let staged = EditOp::AddStep {
                slide: index.into(),
                at: None,
                action: StepAction::reveal(".added"),
            };
            assert_eq!(
                parse(&edited(source, &staged)).slides[index].steps.actions.last(),
                Some(&StepAction::reveal(".added")),
                "{staged:?} on {source:?}"
            );
        }
    }
}

#[test]
fn an_inserted_slide_lands_where_it_was_asked_for_and_displaces_nothing() {
    for source in corpus() {
        let before = written_titles(source);

        for at in 0..=before.len() {
            let op = EditOp::InsertSlide { at, body: "# Inserted".into() };
            let mut expected = before.clone();
            expected.insert(at, "Inserted".to_string());

            assert_eq!(written_titles(&edited(source, &op)), expected, "{op:?} on {source:?}");
        }
    }
}

#[test]
fn a_duplicated_slide_lands_after_its_source_and_keeps_its_words() {
    for source in corpus() {
        let before = written_titles(source);
        if before.is_empty() {
            continue;
        }

        for from in [0, before.len() - 1] {
            let op = EditOp::DuplicateSlide { slide: from.into(), after: None };
            let mut expected = before.clone();
            expected.insert(from + 1, before[from].clone());

            assert_eq!(written_titles(&edited(source, &op)), expected, "{op:?} on {source:?}");
        }
    }
}

#[test]
fn a_duplicated_slide_can_land_after_another_slide_and_keeps_its_words() {
    for source in corpus() {
        let before = written_titles(source);
        if before.len() < 2 {
            continue;
        }

        for (from, after) in [(0, before.len() - 1), (before.len() - 1, 0)] {
            let op = EditOp::DuplicateSlide { slide: from.into(), after: Some(after.into()) };
            let mut expected = before.clone();
            expected.insert(after + 1, before[from].clone());

            assert_eq!(written_titles(&edited(source, &op)), expected, "{op:?} on {source:?}");
        }
    }
}

#[test]
fn a_moved_slide_lands_where_the_operation_said_and_arrives_intact() {
    for source in corpus() {
        let before = written_titles(source);
        if before.len() < 2 {
            continue;
        }

        for (from, to) in [(0, before.len() - 1), (before.len() - 1, 0), (0, 1)] {
            let op = EditOp::MoveSlide { slide: from.into(), to };
            let mut expected = before.clone();
            let moved = expected.remove(from);
            expected.insert(to, moved);

            assert_eq!(written_titles(&edited(source, &op)), expected, "{op:?} on {source:?}");
        }
    }
}

#[test]
fn the_blocks_in_the_file_are_the_blocks_on_the_slide() {
    // The renderer writes each block's index — its index in the *model* — onto
    // the page, and a drag sends that number back as the block to move. So the
    // list an operation counts in the file has to be the list the author was
    // looking at. Two things separate them: notes come out of the content
    // before a slide is rendered, and a step marker becomes an anchor that is
    // folded into the block it stages.
    for source in corpus() {
        let spans = slidx_edit::slide_spans(source, &DeckParseOptions::default());

        for (index, slide) in parse(source).slides.iter().enumerate() {
            let found = slidx_core::find_blocks(spans[index].body.slice(source));

            assert_eq!(
                found.len(),
                slide.blocks.len(),
                "slide {index} of {source:?} counts {} blocks in the file and {} on the slide",
                found.len(),
                slide.blocks.len()
            );
        }
    }
}

#[test]
fn a_moved_block_lands_where_the_operation_said_and_arrives_intact() {
    for source in corpus() {
        for (index, slide) in parse(source).slides.iter().enumerate() {
            let before = block_text(slide);
            if before.len() < 2 {
                continue;
            }

            for (from, to) in [(0, before.len() - 1), (before.len() - 1, 0), (0, 1)] {
                let op =
                    EditOp::MoveBlock { slide: index.into(), block: from.into(), to, region: None };
                let mut expected = before.clone();
                let moved = expected.remove(from);
                expected.insert(to, moved);

                assert_eq!(
                    block_text(&parse(&edited(source, &op)).slides[index]),
                    expected,
                    "{op:?} on {source:?}"
                );
            }
        }
    }
}

/// Each block of a slide, as the model has it.
fn block_text(slide: &slidx_core::Slide) -> Vec<String> {
    slide.blocks.iter().map(|block| block.span.slice(&slide.content).to_string()).collect()
}

#[test]
fn a_removed_slide_is_the_only_one_that_goes() {
    for source in corpus() {
        let before = written_titles(source);
        if before.len() < 2 {
            continue;
        }

        for index in 0..before.len() {
            let op = EditOp::RemoveSlide { slide: index.into() };
            let mut expected = before.clone();
            expected.remove(index);

            assert_eq!(written_titles(&edited(source, &op)), expected, "{op:?} on {source:?}");
        }
    }
}

#[test]
fn an_edit_is_exactly_undone_by_its_inverse() {
    for source in corpus() {
        for op in operations(source) {
            let edit = plan(source, &DeckParseOptions::default(), &op).unwrap();
            let changed = edit.apply(source);

            assert_eq!(
                edit.invert(source).apply(&changed),
                source,
                "{op:?} on {source:?} could not be taken back"
            );
        }
    }
}

// -------------------------------------------------------------- idempotence

#[test]
fn setting_something_to_what_it_already_says_is_not_an_edit() {
    for source in corpus() {
        let deck = parse(source);

        let located = slidx_edit::slide_spans(source, &DeckParseOptions::default());

        for (index, slide) in deck.slides.iter().enumerate() {
            let mut ops = vec![EditOp::SetNotes { slide: index.into(), notes: slide.notes_text() }];

            if let Some(title) = &slide.title {
                ops.push(EditOp::SetHeading { slide: index.into(), text: title.clone() });
            }
            for (property, value) in &slide.style {
                ops.push(EditOp::SetStyle {
                    slide: index.into(),
                    property: property.clone(),
                    value: Some(value.clone()),
                });
            }

            // A block retyped with the words already in it. The editor sends
            // the whole run rather than a diff, so this is the shape of every
            // keystroke that ended up changing nothing.
            let body = located[index].body.slice(source);
            for block in &located[index].blocks {
                ops.push(EditOp::SetText {
                    slide: index.into(),
                    range: block.span,
                    text: block.span.slice(body).to_string(),
                });
            }
            if !slide.notes.is_empty() {
                // Rewriting a note with its own words leaves the comment alone.
                ops.push(EditOp::SetNotes { slide: index.into(), notes: slide.notes[0].clone() });
            }

            for op in ops {
                let edit = plan(source, &DeckParseOptions::default(), &op).unwrap();
                assert!(edit.is_empty(), "{op:?} on {source:?} planned {:?}", edit.splices());
            }
        }
    }
}

#[test]
fn doing_a_set_twice_is_the_same_as_doing_it_once() {
    for source in corpus() {
        let sets = [
            EditOp::SetHeading { slide: 0.into(), text: "Settled".into() },
            EditOp::SetBody { slide: 0.into(), body: "# Settled\n\nBody.".into() },
            EditOp::SetField { slide: 0.into(), key: "theme".into(), value: "terminal".into() },
            EditOp::SetStyle {
                slide: 0.into(),
                property: "layout".into(),
                value: Some("aside".into()),
            },
            EditOp::SetNotes { slide: 0.into(), notes: "settled".into() },
        ];

        for op in sets {
            let once = edited(source, &op);
            assert_eq!(edited(&once, &op), once, "{op:?} on {source:?} did not settle");
        }
    }
}

// ------------------------------------------------------------------ safety

#[test]
fn an_operation_naming_something_that_is_not_there_is_an_error_not_a_crash() {
    let missing: Vec<EditOp> = vec![
        EditOp::SetBody { slide: 99.into(), body: "x".into() },
        EditOp::SetHeading { slide: SlideRef::Id("nope".into()), text: "x".into() },
        EditOp::InsertSlide { at: 99, body: "x".into() },
        EditOp::DuplicateSlide { slide: 99.into(), after: None },
        EditOp::DuplicateSlide { slide: 0.into(), after: Some(99.into()) },
        EditOp::RemoveSlide { slide: 99.into() },
        EditOp::MoveSlide { slide: 0.into(), to: 99 },
        EditOp::MoveSlide { slide: 99.into(), to: 0 },
        EditOp::SetField { slide: 99.into(), key: "a".into(), value: "b".into() },
        EditOp::SetStyle {
            slide: 99.into(),
            property: "layout".into(),
            value: Some("aside".into()),
        },
        EditOp::AddMark {
            slide: 0.into(),
            range: (900..1000).into(),
            attributes: Default::default(),
        },
        // Backwards, which a selection dragged right to left could produce.
        EditOp::AddMark {
            slide: 0.into(),
            range: ByteSpan::new(5, 1),
            attributes: Default::default(),
        },
        EditOp::SetText { slide: 99.into(), range: ByteSpan::new(0, 1), text: "x".into() },
        EditOp::SetText { slide: 0.into(), range: (900..1000).into(), text: "x".into() },
        // Backwards, which a selection dragged right to left could produce.
        EditOp::SetText { slide: 0.into(), range: ByteSpan::new(5, 1), text: "x".into() },
        EditOp::SetMark { slide: 0.into(), mark: 99.into(), attributes: Default::default() },
        EditOp::RemoveMark { slide: 0.into(), mark: "gone".into() },
        EditOp::AddStep { slide: 99.into(), at: None, action: StepAction::reveal(".a") },
        EditOp::RemoveStep { slide: 0.into(), index: 99 },
        EditOp::SetNotes { slide: 99.into(), notes: "x".into() },
        EditOp::SetBlockAttributes {
            slide: 0.into(),
            block: 99.into(),
            attributes: Default::default(),
        },
        EditOp::SetBlockAttributes {
            slide: 0.into(),
            block: "gone".into(),
            attributes: Default::default(),
        },
        EditOp::MoveBlock { slide: 0.into(), block: 99.into(), to: 0, region: None },
        EditOp::SetBlockWidth { slide: 0.into(), block: 99.into(), width: BlockWidth::Half },
        EditOp::SetBlockWidth { slide: 99.into(), block: 0.into(), width: BlockWidth::Half },
        // A drop target one past the last block, which is where the editor
        // aims when an author drags something to the bottom of a region.
        EditOp::MoveBlock { slide: 0.into(), block: 0.into(), to: 99, region: Some("side".into()) },
    ];

    for source in corpus() {
        for op in &missing {
            let planned = plan(source, &DeckParseOptions::default(), op);

            if let Ok(edit) = planned {
                // Some of these are legitimate on some sources — index 0 exists
                // everywhere. What must never happen is a source that stops
                // being a deck.
                assert!(!parse(&edit.apply(source)).diagnostics.has_blocking());
            }
        }
    }
}

#[test]
fn a_range_that_would_cut_a_character_in_half_is_refused() {
    let source = "# One\n\n日本語のテキスト\n";
    let op = EditOp::AddMark {
        slide: 0.into(),
        range: (7..9).into(),
        attributes: MarkAttributes::default().with_class("accent"),
    };

    assert!(plan(source, &DeckParseOptions::default(), &op).is_err());
}

#[test]
fn every_operation_on_every_deck_in_the_corpus_leaves_a_deck_behind() {
    for source in corpus() {
        for op in operations(source) {
            let result = edited(source, &op);
            let deck = parse(&result);

            assert!(!deck.slides.is_empty(), "{op:?} on {source:?} left no slides");
            assert!(
                !deck.diagnostics.has_blocking(),
                "{op:?} on {source:?} left {:?}",
                deck.diagnostics
            );
        }
    }
}
