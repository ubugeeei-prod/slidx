//! Editing a deck through `slidx mcp`, over the wire.
//!
//! The claim these tests hold is the reason this server exists rather than
//! letting an agent open the file: **an operation changes exactly what it names
//! and every other byte is untouched.**
//!
//! Two of them are the load-bearing ones.
//!
//! `a_run_of_edits_undone_leaves_every_file_byte_identical` drives a sequence of
//! tool calls that touches headings, notes, frontmatter, marks, steps and the
//! slide order, undoes all of them through the `undo` tool, and compares the
//! files with the bytes they started with. Nothing else an agent has can do that:
//! an editing surface that writes whole files has to keep a copy to restore, and
//! restoring a copy is not the same as reversing a change.
//!
//! `an_edit_leaves_the_authors_own_formatting_alone` is the other half. It edits a
//! deck written the way people actually write them — three spaces after the hash,
//! `*` bullets, a hand-wrapped paragraph, a blank line too many — and asserts
//! every one of those survives. A serialiser would regularise all of it, which is
//! invisible on a slide and enormous in a diff.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use slidx_cli::mcp::{self, Session, Workspace};

/// A scratch project that cleans up after itself.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("slidx-mcped-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("slides")).expect("a scratch project");
        Self(path)
    }

    fn slide(&self, name: &str, body: &str) {
        fs::write(self.0.join("slides").join(name), body).expect("write");
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn deck(&self) -> String {
        self.0.display().to_string()
    }

    /// Every slide file and its bytes, as a snapshot to compare against.
    fn files(&self) -> BTreeMap<String, String> {
        let mut found = BTreeMap::new();

        for entry in fs::read_dir(self.0.join("slides")).expect("a slides directory") {
            let path = entry.expect("an entry").path();
            let name = path.file_name().expect("a name").to_string_lossy().into_owned();
            found.insert(name, fs::read_to_string(&path).expect("read"));
        }

        found
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.0.join("slides").join(name)).unwrap_or_default()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A session that may write, over one project.
fn writing(scratch: &Scratch) -> Session {
    Session::new(
        Workspace::new(vec![scratch.path().to_path_buf()])
            .with_index(scratch.path().join("no-index.json"))
            .writing(),
    )
}

fn read_only(scratch: &Scratch) -> Session {
    Session::new(
        Workspace::new(vec![scratch.path().to_path_buf()])
            .with_index(scratch.path().join("no-index.json")),
    )
}

/// Runs a recorded exchange against a session and returns the frames.
fn talk(session: &mut Session, script: &[String]) -> Vec<Value> {
    let input: String = script.iter().map(|line| format!("{line}\n")).collect();
    let mut output = Vec::new();

    mcp::serve(&mut input.as_bytes(), &mut output, session).expect("the session ran");

    String::from_utf8(output)
        .expect("frames are UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|_| panic!("not a frame: {line}")))
        .collect()
}

fn hello() -> String {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": mcp::PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "a-client", "version": "1.0.0" },
        },
    })
    .to_string()
}

fn call(id: i64, name: &str, arguments: Value) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments },
    })
    .to_string()
}

/// The result of the call with this id, insisting it succeeded.
fn result(frames: &[Value], id: i64) -> Value {
    let frame = frames
        .iter()
        .find(|frame| frame["id"] == id)
        .unwrap_or_else(|| panic!("no frame answered {id}: {frames:#?}"));

    assert!(frame["error"].is_null(), "id {id} was a protocol error: {frame}");
    assert_eq!(frame["result"]["isError"], false, "id {id} failed: {frame}");

    frame["result"].clone()
}

/// A deck written the way people actually write them.
const AS_WRITTEN: &str = "---\ntitle: Making Decks Fast\nduration: 20m\ntheme: editorial\n---\n\n\
#   Making Decks Fast\n\n\
*  the parser\n*  the linter\n\n\
A paragraph the author\nwrapped by hand.\n";

#[test]
fn a_run_of_edits_undone_leaves_every_file_byte_identical() {
    // The claim this whole server rests on. Six changes across three files,
    // every one of them reversed by the edit that came back with it, and then
    // the deck compared byte for byte with what it was.
    let scratch = Scratch::new("undo-run");
    scratch.slide("0001.md", AS_WRITTEN);
    scratch.slide("0002.md", "# What we will cover\n\n- why the parser matters\n");
    scratch.slide("0003.md", "# Results\n\nLatency dropped to 38ms.\n");
    let before = scratch.files();

    let deck = scratch.deck();
    let mut session = writing(&scratch);

    let edits = vec![
        hello(),
        call(2, "set_heading", json!({ "deck": deck, "slide": 1, "text": "The plan" })),
        call(
            3,
            "set_notes",
            json!({ "deck": deck, "slide": 0, "notes": "Open with the outcome." }),
        ),
        call(
            4,
            "set_field",
            json!({ "deck": deck, "slide": 0, "key": "event", "value": "SlidxConf 2026" }),
        ),
        call(5, "add_mark", json!({ "deck": deck, "slide": 2, "text": "38ms", "key": "latency" })),
        call(
            6,
            "add_step",
            json!({ "deck": deck, "slide": 2, "action": "reveal", "target": "#latency" }),
        ),
        call(7, "move_slide", json!({ "deck": deck, "slide": 2, "to": 0 })),
    ];

    let frames = talk(&mut session, &edits);
    for id in 2..=7 {
        let answer = result(&frames, id);
        assert_eq!(answer["structuredContent"]["redundant"], false, "id {id} changed nothing");
        assert!(
            !answer["structuredContent"]["inverse"].as_array().expect("an inverse").is_empty(),
            "id {id} came back without the edit that reverses it"
        );
    }

    assert_eq!(session.undo_depth(), 6, "six changes on the stack");
    assert_ne!(scratch.files(), before, "and the deck really did change");

    let undos: Vec<String> = (10..16).map(|id| call(id, "undo", json!({}))).collect();
    let frames = talk(&mut session, &undos);
    for id in 10..16 {
        result(&frames, id);
    }

    assert_eq!(session.undo_depth(), 0);
    assert_eq!(scratch.files(), before, "every byte back where it was");
}

#[test]
fn an_edit_leaves_the_authors_own_formatting_alone() {
    // Three spaces after the hash, `*` bullets, a hand-wrapped paragraph. A
    // serialiser would regularise every one of them: invisible on the slide,
    // enormous in the diff, and the end of the promise that a deck is still
    // Markdown the author owns.
    let scratch = Scratch::new("formatting");
    scratch.slide("0001.md", AS_WRITTEN);
    let deck = scratch.deck();

    let frames = talk(
        &mut writing(&scratch),
        &[
            hello(),
            call(2, "set_heading", json!({ "deck": deck, "slide": 0, "text": "Fast Decks" })),
        ],
    );
    result(&frames, 2);

    let after = scratch.read("0001.md");

    assert_eq!(after, AS_WRITTEN.replace("Making Decks Fast\n\n*", "Fast Decks\n\n*"));
    assert!(after.contains("#   Fast Decks"), "the three spaces survive: {after:?}");
    assert!(after.contains("*  the parser"), "the bullets survive");
    assert!(after.contains("A paragraph the author\nwrapped by hand."), "the wrapping survives");
    assert!(after.contains("title: Making Decks Fast"), "the frontmatter is untouched");
}

#[test]
fn writing_a_frontmatter_key_leaves_the_others_in_the_order_the_author_wrote_them() {
    // A serialiser sorts keys. That is a diff across the whole block for a
    // one-word change, and an author stops trusting the tool with their file.
    let scratch = Scratch::new("frontmatter");
    scratch.slide("0001.md", AS_WRITTEN);
    let deck = scratch.deck();

    let frames = talk(
        &mut writing(&scratch),
        &[
            hello(),
            call(
                2,
                "set_field",
                json!({ "deck": deck, "slide": 0, "key": "duration", "value": "25m" }),
            ),
        ],
    );
    result(&frames, 2);

    let after = scratch.read("0001.md");
    let keys: Vec<&str> = after
        .lines()
        .skip(1)
        .take_while(|line| *line != "---")
        .filter_map(|line| line.split(':').next())
        .collect();

    assert_eq!(keys, ["title", "duration", "theme"]);
    assert!(after.contains("duration: 25m"));
}

#[test]
fn a_deck_kept_as_one_file_per_slide_only_writes_the_files_that_changed() {
    // Not "written with identical content" — not written. The difference is a
    // modification time that never moves and a watcher that never fires.
    let scratch = Scratch::new("touched");
    scratch.slide("0001.md", "# One\n");
    scratch.slide("0002.md", "# Two\n");
    scratch.slide("0003.md", "# Three\n");
    let deck = scratch.deck();

    let frames = talk(
        &mut writing(&scratch),
        &[hello(), call(2, "set_heading", json!({ "deck": deck, "slide": 1, "text": "Second" }))],
    );

    assert_eq!(result(&frames, 2)["structuredContent"]["changed"], json!(["0002.md"]));
    assert_eq!(scratch.read("0001.md"), "# One\n");
    assert_eq!(scratch.read("0003.md"), "# Three\n");
}

#[test]
fn asking_for_what_the_deck_already_says_writes_nothing_and_says_so() {
    // Idempotence is a property of slidx_edit rather than of each operation, and
    // an agent has to be able to tell "already true" from "done".
    let scratch = Scratch::new("idempotent");
    scratch.slide("0001.md", "# One\n");
    let deck = scratch.deck();

    let mut session = writing(&scratch);
    let frames = talk(
        &mut session,
        &[hello(), call(2, "set_heading", json!({ "deck": deck, "slide": 0, "text": "One" }))],
    );
    let answer = result(&frames, 2);

    assert_eq!(answer["structuredContent"]["redundant"], true);
    assert_eq!(answer["structuredContent"]["changed"], json!([]));
    assert!(answer["content"][0]["text"].as_str().expect("a text").contains("already said this"));
    assert_eq!(session.undo_depth(), 0, "there is nothing to take back");
}

#[test]
fn a_read_only_server_neither_lists_nor_runs_an_operation() {
    let scratch = Scratch::new("read-only");
    scratch.slide("0001.md", "# One\n");
    let deck = scratch.deck();
    let before = scratch.files();

    let frames = talk(
        &mut read_only(&scratch),
        &[
            hello(),
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#.to_string(),
            call(3, "set_heading", json!({ "deck": deck, "slide": 0, "text": "Renamed" })),
        ],
    );

    let listed: Vec<&str> = frames[1]["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|tool| tool["name"].as_str().unwrap_or_default())
        .collect();
    assert!(!listed.contains(&"set_heading"), "{listed:?}");
    assert!(!listed.contains(&"undo"), "{listed:?}");

    assert_eq!(frames[2]["result"]["isError"], true);
    let reason = frames[2]["result"]["content"][0]["text"].as_str().expect("a reason");
    assert!(reason.contains("read-only"), "{reason}");
    assert!(reason.contains("--write"), "{reason}");
    assert_eq!(scratch.files(), before, "and nothing was written");
}

#[test]
fn a_writing_server_offers_every_operation_slidx_edit_defines() {
    // The closed set, served. An operation nothing can reach is a gesture an
    // agent does by rewriting the file instead.
    let scratch = Scratch::new("surface");
    scratch.slide("0001.md", "# One\n");

    let frames = talk(
        &mut writing(&scratch),
        &[hello(), r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#.to_string()],
    );
    let listed: Vec<&str> = frames[1]["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|tool| tool["name"].as_str().unwrap_or_default())
        .collect();

    for expected in [
        "set_heading",
        "set_body",
        "set_notes",
        "set_field",
        "insert_slide",
        "remove_slide",
        "move_slide",
        "add_mark",
        "set_mark",
        "remove_mark",
        "add_step",
        "remove_step",
        "undo",
    ] {
        assert!(listed.contains(&expected), "{expected} is not offered: {listed:?}");
    }
}

#[test]
fn no_tool_offers_a_way_to_write_raw_text_to_a_file() {
    // The property that makes this worth using at all. Every mutation names a
    // slide and a change to it; nothing names a path and bytes.
    let scratch = Scratch::new("no-writer");
    scratch.slide("0001.md", "# One\n");

    let frames = talk(
        &mut writing(&scratch),
        &[hello(), r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#.to_string()],
    );

    for tool in frames[1]["result"]["tools"].as_array().expect("tools") {
        let properties = tool["inputSchema"]["properties"].clone();

        // Each of these names a file's bytes. `deck` is the only path-shaped
        // argument any tool takes, and it says *which* deck rather than what to
        // put in it; `text` and `body` name a heading and a slide, which are
        // things the operation set knows about.
        for forbidden in ["path", "file", "filename", "contents", "source"] {
            assert!(properties[forbidden].is_null(), "{} takes `{forbidden}`", tool["name"]);
        }

        assert!(
            properties["deck"].is_null()
                || properties["deck"]["description"]
                    .as_str()
                    .is_some_and(|text| text.contains("directory this server")),
            "{} takes a deck path without saying it is confined",
            tool["name"]
        );
    }
}

#[test]
fn text_that_appears_twice_is_refused_rather_than_marked_at_a_guess() {
    // Marking the wrong three words leaves nothing in the file saying which was
    // meant, so the answer says how many there are and how to choose.
    let scratch = Scratch::new("ambiguous");
    scratch.slide("0001.md", "# One\n\nfast, and then fast again\n");
    let deck = scratch.deck();
    let before = scratch.files();

    let frames = talk(
        &mut writing(&scratch),
        &[
            hello(),
            call(2, "add_mark", json!({ "deck": deck, "slide": 0, "text": "fast", "key": "a" })),
        ],
    );

    assert_eq!(frames[1]["result"]["isError"], true);
    let reason = frames[1]["result"]["content"][0]["text"].as_str().expect("a reason");
    assert!(reason.contains("appears 2 times"), "{reason}");
    assert!(reason.contains("occurrence"), "{reason}");
    assert_eq!(scratch.files(), before);
}

#[test]
fn a_deck_the_index_knows_about_can_be_read_and_not_written() {
    // A server pointed at one project must not rewrite a talk somebody gave last
    // year because a path in front of it mentioned one.
    let here = Scratch::new("here");
    here.slide("0001.md", "# One\n");
    let elsewhere = Scratch::new("elsewhere");
    elsewhere.slide("0001.md", "# Last year\n");
    let before = elsewhere.files();

    let index = here.path().join("index.json");
    let mut session = Session::new(
        Workspace::new(vec![here.path().to_path_buf()]).with_index(index.clone()).writing(),
    );

    // The index fills itself from any slidx command, which is what makes this
    // path reachable at all.
    slidx_cli::index::remember(&index, slidx_cli::index::Entry::new(elsewhere.path()));

    let deck = elsewhere.deck();
    let frames = talk(
        &mut session,
        &[
            hello(),
            call(2, "lint_deck", json!({ "deck": deck })),
            call(3, "set_heading", json!({ "deck": deck, "slide": 0, "text": "Renamed" })),
        ],
    );

    result(&frames, 2);
    assert_eq!(frames[2]["result"]["isError"], true);
    let reason = frames[2]["result"]["content"][0]["text"].as_str().expect("a reason");
    assert!(reason.contains("read but not written"), "{reason}");
    assert_eq!(elsewhere.files(), before);
}

#[test]
fn formatting_is_a_splice_like_anything_else_and_undoes_exactly() {
    // `slidx_fmt` produces the same invertible edit an operation does, which is
    // what having one representation of a change buys: an agent can normalise a
    // deck it does not own and take it back byte for byte.
    let scratch = Scratch::new("format");
    scratch.slide(
        "0001.md",
        "---\n  theme:   editorial\n  title:  A talk\n---\n\n\
         # One\n\n\
         The result was [fast]{.accent #hero}.\n\n\
         A paragraph the author\nwrapped by hand, with *  odd  * spacing inside it.\n",
    );
    let before = scratch.files();
    let deck = scratch.deck();

    let mut session = writing(&scratch);
    let frames = talk(&mut session, &[hello(), call(2, "format_deck", json!({ "deck": deck }))]);
    let answer = result(&frames, 2);

    assert_eq!(answer["structuredContent"]["redundant"], false);
    let formatted = scratch.read("0001.md");
    assert_ne!(formatted, before["0001.md"]);
    assert!(formatted.contains("[fast]{#hero .accent}"), "the mark is normalised: {formatted}");
    assert!(
        formatted.contains("A paragraph the author\nwrapped by hand, with *  odd  * spacing"),
        "the author's prose is byte for byte: {formatted}"
    );

    let frames = talk(&mut session, &[call(3, "undo", json!({}))]);
    result(&frames, 3);

    assert_eq!(scratch.files(), before);
}

#[test]
fn undoing_with_nothing_to_undo_says_it_does_not_reach_past_the_server_starting() {
    let scratch = Scratch::new("nothing");
    scratch.slide("0001.md", "# One\n");

    let frames = talk(&mut writing(&scratch), &[hello(), call(2, "undo", json!({}))]);

    assert_eq!(frames[1]["result"]["isError"], true);
    assert!(frames[1]["result"]["content"][0]["text"]
        .as_str()
        .expect("a reason")
        .contains("version control"));
}

#[test]
fn a_slide_removed_and_then_restored_puts_its_file_back() {
    // The hardest thing for an undo to get right in a deck kept as one file per
    // slide: the file was deleted, so restoring the slide has to recreate it.
    let scratch = Scratch::new("restore");
    scratch.slide("0001.md", "# One\n");
    scratch.slide("0002.md", "# Two\n");
    scratch.slide("0003.md", "# Three\n");
    let before = scratch.files();
    let deck = scratch.deck();

    let mut session = writing(&scratch);
    let frames = talk(
        &mut session,
        &[hello(), call(2, "remove_slide", json!({ "deck": deck, "slide": 1 }))],
    );
    result(&frames, 2);
    assert!(!scratch.path().join("slides/0002.md").exists());

    let frames = talk(&mut session, &[call(3, "undo", json!({}))]);
    result(&frames, 3);

    assert_eq!(scratch.files(), before, "the file and its bytes are back");
}

#[test]
fn a_japanese_deck_is_edited_and_undone_by_bytes() {
    // Three bytes per kanji. A range counted in characters would land inside one
    // and produce a file that is not valid UTF-8.
    let scratch = Scratch::new("japanese");
    scratch.slide("0001.md", "---\ntitle: 高速なデッキ\n---\n\n# 導入\n\n速度が上がりました。\n");
    let before = scratch.files();
    let deck = scratch.deck();

    let mut session = writing(&scratch);
    let frames = talk(
        &mut session,
        &[
            hello(),
            call(
                2,
                "add_mark",
                json!({ "deck": deck, "slide": 0, "text": "速度", "key": "speed" }),
            ),
            call(3, "set_heading", json!({ "deck": deck, "slide": 0, "text": "はじめに" })),
        ],
    );
    result(&frames, 2);
    result(&frames, 3);

    assert!(scratch.read("0001.md").contains("[速度]{#speed}"));

    let frames = talk(&mut session, &[call(4, "undo", json!({})), call(5, "undo", json!({}))]);
    result(&frames, 4);
    result(&frames, 5);

    assert_eq!(scratch.files(), before);
}
