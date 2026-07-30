//! The handful of things a speaker actually asks for.
//!
//! A prompt is worth serving only where it knows something a generic one does
//! not. "Write speaker notes" is a sentence anybody can type; what slidx can add
//! is that notes are what the speaker *says* rather than a second copy of the
//! slide, that they drive the spoken-length estimate the timing report compares
//! against the slot, and that the way to write them is `set_notes` rather than
//! rewriting the file.
//!
//! So each of these carries the deck's own content and slidx's own rules, and
//! there are three rather than thirty.
//!
//! ## The content is embedded, not described
//!
//! A prompt that told a model to go and read the slide costs a round trip and
//! gets the wrong slide half the time. These read the deck and put the slide in
//! the message, which is the thing a prompt can do that a tool description
//! cannot.

use serde_json::{json, Value};

use super::workspace::Workspace;

/// What a prompt expands to: a one-line description, and the message itself.
///
/// A pair rather than a struct because the description is a label a client shows
/// beside the prompt in a picker and the text is what the model reads, and
/// nothing else ever needs to tell them apart.
type Filled = Result<(String, String), String>;

/// One prompt, as a client lists it and as the server fills it in.
#[derive(Debug, Clone, Copy)]
pub struct Prompt {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    /// The arguments, in the order a client shows them.
    pub arguments: &'static [Argument],
    pub build: fn(&Workspace, &Value) -> Filled,
}

/// One argument a prompt takes.
#[derive(Debug, Clone, Copy)]
pub struct Argument {
    pub name: &'static str,
    pub description: &'static str,
    pub required: bool,
}

impl Prompt {
    /// The descriptor a client lists.
    pub fn describe(&self) -> Value {
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "arguments": self
                .arguments
                .iter()
                .map(|argument| json!({
                    "name": argument.name,
                    "description": argument.description,
                    "required": argument.required,
                }))
                .collect::<Vec<_>>(),
        })
    }
}

pub const ALL: &[Prompt] = &[
    Prompt {
        name: "write_notes",
        title: "Write what the speaker says over a slide",
        description: "\
Drafts speaker notes for one slide, in slidx's terms: what the speaker says \
rather than a second copy of what the audience reads, against the budget that \
slide was given.",
        arguments: &[
            Argument { name: "deck", description: "The deck.", required: true },
            Argument {
                name: "slide",
                description: "The slide, counting from zero.",
                required: true,
            },
        ],
        build: write_notes,
    },
    Prompt {
        name: "split_slide",
        title: "Split a slide that is too full",
        description: "\
Turns one overloaded slide into two, without reformatting anything the author \
wrote and without losing the marks and steps that point into it.",
        arguments: &[
            Argument { name: "deck", description: "The deck.", required: true },
            Argument {
                name: "slide",
                description: "The slide, counting from zero.",
                required: true,
            },
        ],
        build: split_slide,
    },
    Prompt {
        name: "check_the_room",
        title: "Check a deck for the room it will be shown in",
        description: "\
Reads the deck against what a conference room does to a slide, and says what to \
fix first — including the things the linter cannot see from the source.",
        arguments: &[
            Argument { name: "deck", description: "The deck.", required: true },
            Argument {
                name: "venue",
                description: "\
Anything known about the room: how far the back row is, whether the projector is \
washed out, whether there is a caption strip along the bottom.",
                required: false,
            },
        ],
        build: check_the_room,
    },
];

pub fn find(name: &str) -> Option<&'static Prompt> {
    ALL.iter().find(|prompt| prompt.name == name)
}

/// The messages one prompt expands to.
pub fn get(workspace: &Workspace, name: &str, arguments: &Value) -> Result<Value, String> {
    let prompt = find(name).ok_or_else(|| {
        format!("There is no prompt called `{name}`. Ask `prompts/list` for the ones there are.")
    })?;

    let (description, text) = (prompt.build)(workspace, arguments)?;

    Ok(json!({
        "description": description,
        "messages": [{ "role": "user", "content": { "type": "text", "text": text } }],
    }))
}

fn write_notes(workspace: &Workspace, arguments: &Value) -> Filled {
    let (reading, index, source) = slide_of(workspace, arguments)?;
    let slide = &reading.deck.slides[index];

    let budget = match slide.budget_seconds {
        Some(seconds) => format!("{seconds} seconds, which it declares in `budget:`"),
        None => match reading.deck.meta.duration_seconds {
            Some(total) => format!(
                "unstated; the deck has {total} seconds for {} slides, so about {} each",
                reading.deck.slides.len(),
                total as usize / reading.deck.slides.len().max(1)
            ),
            None => "unstated, and so is the deck's slot".to_string(),
        },
    };

    let existing = if slide.notes.is_empty() {
        "It has no notes yet.".to_string()
    } else {
        format!("It already says:\n\n{}", slide.notes_text())
    };

    Ok((
        format!("Speaker notes for slide {index} of {}", reading.label),
        format!(
            "Write the speaker notes for this slide of \"{}\".\n\n\
             The slide, as the author wrote it:\n\n```markdown\n{source}\n```\n\n\
             {existing}\n\n\
             Time on this slide: {budget}.\n\n\
             What notes are in slidx:\n\n\
             - They are what the speaker SAYS. The slide is already on the wall behind them; \
             what they cannot see is what they meant to say about it. Notes that restate the \
             bullets are notes nobody can use.\n\
             - They drive the spoken-length estimate the timing report compares against the \
             slot, at 150 words a minute and 300 characters a minute for CJK. So write the \
             length the slide is actually worth, not a paragraph for a title slide.\n\
             - Write them with the `set_notes` tool, which splices them into the file. Do not \
             write the file yourself.\n\n\
             Write in the language the deck is written in.",
            reading.deck.meta.display_title(),
        ),
    ))
}

fn split_slide(workspace: &Workspace, arguments: &Value) -> Filled {
    let (reading, index, source) = slide_of(workspace, arguments)?;
    let slide = &reading.deck.slides[index];

    let marks = if slide.marks.is_empty() {
        "It has no marks.".to_string()
    } else {
        format!(
            "It has {} mark(s): {}. Anything a step targets has to end up on the same slide as \
             the step that targets it.",
            slide.marks.len(),
            slide.marks.iter().map(|mark| mark.to_source()).collect::<Vec<_>>().join(", ")
        )
    };

    let steps = match slide.stop_count() {
        0 | 1 => "It has no steps.".to_string(),
        stops => format!(
            "It has {} stops. A step list belongs to one slide, so splitting means deciding \
             which stops go with which half — the `steps:` key does not travel on its own.",
            stops - 1
        ),
    };

    Ok((
        format!("Split slide {index} of {}", reading.label),
        format!(
            "This slide is too full. Split it in two.\n\n\
             The slide, as the author wrote it:\n\n```markdown\n{source}\n```\n\n\
             {marks}\n\n{steps}\n\n\
             How to do it here:\n\n\
             1. Decide where the break goes. A slide carries one message; the break is where \
             the second one starts, not halfway down the bullets.\n\
             2. `insert_slide` at {} with the second half's Markdown.\n\
             3. `set_body` on slide {index} with what is left.\n\n\
             Rules that are not negotiable:\n\n\
             - REUSE THE AUTHOR'S BYTES. Copy their lines across as they are — their bullet \
             markers, their spacing, their line wrapping. Do not tidy anything on the way. A \
             split that also reformats is a diff nobody can review.\n\
             - Give the new slide a heading that says what it is about. \"Continued\" is not one.\n\
             - Move the marks with the text they wrap, and check that every `steps:` target \
             still exists on the slide whose steps they are.\n\
             - Both halves keep the deck's language.\n\n\
             Say what you are going to do before you do it.",
            index + 1,
        ),
    ))
}

fn check_the_room(workspace: &Workspace, arguments: &Value) -> Filled {
    let path = super::tool::args::required(arguments, "deck", "the deck to check.")?;
    let reading = workspace.read_deck(&path, None)?;
    let venue = super::tool::args::text(arguments, "venue").unwrap_or("nothing was said about it");

    let slot = match reading.deck.meta.duration_seconds {
        Some(seconds) => format!("{} minutes", seconds / 60),
        None => "not declared, which the linter will say".to_string(),
    };

    Ok((
        format!("Check {} for the room", reading.label),
        format!(
            "Check this deck for the room it will be shown in.\n\n\
             Deck: {}\nSlides: {}\nSlot: {slot}\nThe room: {venue}\n\n\
             Do this:\n\n\
             1. Run `lint_deck` on it. Every finding carries a concrete next action — read \
             those rather than inventing advice, and report them worst first.\n\
             2. Read the deck's own model or the slides that were flagged, and say which \
             findings actually matter for THIS room. A contrast finding matters more in a \
             bright room; a font size finding matters more in a deep one.\n\
             3. Say what the linter could not check, because a clean run is not a clean deck:\n\
             \x20  - whether content OVERFLOWS is measured in a real browser during \
             `vite build`, not from the source. It is unchecked here, not clean.\n\
             \x20  - whether a demo works, whether the venue's wifi exists, and whether the \
             machine is ready are `slidx doctor` and the day itself.\n\
             \x20  - whether the talk is any good.\n\
             4. If the room's numbers differ from the deck's, name the frontmatter key that \
             would tell slidx — `safeArea:` for a caption strip, `duration:` for the slot.\n\n\
             Do not change anything. Say what you would change and why.",
            reading.deck.meta.display_title(),
            reading.deck.slides.len(),
        ),
    ))
}

/// The deck, the slide index, and the slide's source as the author wrote it.
fn slide_of(
    workspace: &Workspace,
    arguments: &Value,
) -> Result<(super::workspace::Reading, usize, String), String> {
    let path = super::tool::args::required(arguments, "deck", "the deck the slide is in.")?;
    let reading = workspace.read_deck(&path, None)?;

    // Arguments arrive as strings: the protocol's prompt arguments are a string
    // map, so a slide number is text however a client's user typed it.
    let index: usize = super::tool::args::text(arguments, "slide")
        .ok_or_else(|| "`slide` is required: which slide, counting from zero.".to_string())?
        .trim()
        .parse()
        .map_err(|_| "`slide` is a number, counting from zero.".to_string())?;

    if reading.deck.slides.get(index).is_none() {
        return Err(super::resource::deck::missing(index, &reading));
    }

    let options =
        slidx_core::DeckParseOptions { separator: reading.separator.clone(), ..Default::default() };
    let source = slidx_edit::slide_spans(&reading.source, &options)
        .get(index)
        .map(|span| span.content.slice(&reading.source).to_string())
        .unwrap_or_default();

    Ok((reading, index, source))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str, body: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-prompt-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(path.join("slides")).expect("scratch");
            fs::write(path.join("slides/0001.md"), body).expect("write");
            Self(path)
        }

        fn workspace(&self) -> Workspace {
            Workspace::new(vec![self.0.clone()]).with_index(self.0.join("no-index.json"))
        }

        fn deck(&self) -> String {
            self.0.display().to_string()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const DECK: &str = "---\ntitle: Making Decks Fast\nduration: 20m\n---\n\n\
                        #   Making Decks Fast\n\n\
                        *  the parser\n*  the linter\n\n\
                        The result was [3.2x faster]{#result .accent}.\n";

    fn expanded(scratch: &Scratch, name: &str, arguments: Value) -> String {
        get(&scratch.workspace(), name, &arguments).expect("a prompt")["messages"][0]["content"]
            ["text"]
            .as_str()
            .expect("a text")
            .to_string()
    }

    #[test]
    fn every_prompt_says_what_it_is_for_and_what_it_takes() {
        for prompt in ALL {
            let described = prompt.describe();

            assert!(described["description"].as_str().is_some_and(|text| text.len() > 60));
            assert!(!described["arguments"].as_array().expect("arguments").is_empty());
            for argument in described["arguments"].as_array().expect("arguments") {
                assert!(argument["description"].as_str().is_some_and(|text| !text.is_empty()));
            }
        }
    }

    #[test]
    fn a_prompt_carries_the_slide_rather_than_telling_a_model_to_go_and_read_it() {
        // A round trip a prompt can save, and one a model gets wrong half the
        // time by fetching a different slide.
        let scratch = Scratch::new("carries", DECK);
        let text =
            expanded(&scratch, "write_notes", json!({ "deck": scratch.deck(), "slide": "0" }));

        assert!(text.contains("#   Making Decks Fast"), "the author's own bytes: {text}");
        assert!(text.contains("*  the parser"));
    }

    #[test]
    fn the_notes_prompt_states_what_notes_are_for_in_slidx() {
        // The thing a generic "write speaker notes" prompt cannot know, and the
        // reason serving this one is worth anything at all.
        let scratch = Scratch::new("notes", DECK);
        let text =
            expanded(&scratch, "write_notes", json!({ "deck": scratch.deck(), "slide": "0" }));

        assert!(text.contains("what the speaker SAYS"), "{text}");
        assert!(text.contains("spoken-length estimate"), "{text}");
        assert!(text.contains("set_notes"), "it names the tool that writes them");
        assert!(text.contains("Do not write the file yourself"));
    }

    #[test]
    fn the_notes_prompt_works_out_the_time_this_slide_has() {
        // A deck with a declared slot and no per-slide budget still has an
        // answer, and it is the one that decides how much to write.
        let scratch = Scratch::new("budget", DECK);
        let text =
            expanded(&scratch, "write_notes", json!({ "deck": scratch.deck(), "slide": "0" }));

        assert!(text.contains("1200 seconds for 1 slides"), "{text}");
    }

    #[test]
    fn the_split_prompt_names_the_marks_that_have_to_move_with_their_text() {
        // A step whose target ends up on the other slide is an animation that
        // silently does nothing, on stage.
        let scratch = Scratch::new("split", DECK);
        let text =
            expanded(&scratch, "split_slide", json!({ "deck": scratch.deck(), "slide": "0" }));

        assert!(text.contains("[3.2x faster]{#result .accent}"), "{text}");
        assert!(text.contains("same slide as the step that targets it"));
    }

    #[test]
    fn the_split_prompt_forbids_tidying_on_the_way() {
        // The failure this whole server exists to prevent, at the one moment a
        // model is most tempted: it is already rewriting a slide.
        let scratch = Scratch::new("tidy", DECK);
        let text =
            expanded(&scratch, "split_slide", json!({ "deck": scratch.deck(), "slide": "0" }));

        assert!(text.contains("REUSE THE AUTHOR'S BYTES"), "{text}");
        assert!(text.contains("insert_slide"), "it names the operation, not a file write");
    }

    #[test]
    fn the_room_prompt_says_what_a_clean_lint_run_does_not_prove() {
        // "Clean" and "unchecked" look identical in a report, and the difference
        // is a slide whose content is cut off on the day.
        let scratch = Scratch::new("room", DECK);
        let text = expanded(&scratch, "check_the_room", json!({ "deck": scratch.deck() }));

        assert!(text.contains("OVERFLOWS"), "{text}");
        assert!(text.contains("unchecked here, not clean"), "{text}");
        assert!(text.contains("slidx doctor"));
        assert!(text.contains("Do not change anything"));
    }

    #[test]
    fn the_room_prompt_takes_what_is_known_about_the_venue() {
        let scratch = Scratch::new("venue", DECK);
        let text = expanded(
            &scratch,
            "check_the_room",
            json!({ "deck": scratch.deck(), "venue": "a caption strip along the bottom" }),
        );

        assert!(text.contains("a caption strip along the bottom"), "{text}");
    }

    #[test]
    fn a_slide_number_arrives_as_text_because_that_is_what_the_protocol_carries() {
        // Prompt arguments are a string map. A prompt that only accepted a JSON
        // number would fail for every client that sent one properly.
        let scratch = Scratch::new("string", DECK);

        assert!(get(
            &scratch.workspace(),
            "write_notes",
            &json!({ "deck": scratch.deck(), "slide": "0" })
        )
        .is_ok());
    }

    #[test]
    fn a_prompt_that_does_not_exist_names_the_ones_that_do() {
        let scratch = Scratch::new("unknown", DECK);
        let refusal =
            get(&scratch.workspace(), "make_it_pop", &json!({})).expect_err("no such prompt");

        assert!(refusal.contains("prompts/list"), "{refusal}");
    }

    #[test]
    fn a_slide_that_is_not_there_says_how_many_there_are() {
        let scratch = Scratch::new("missing", DECK);
        let refusal = get(
            &scratch.workspace(),
            "write_notes",
            &json!({ "deck": scratch.deck(), "slide": "9" }),
        )
        .expect_err("no such slide");

        assert!(refusal.contains("numbered from zero"), "{refusal}");
    }
}
