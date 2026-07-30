//! Resources and prompts, stated as recorded JSON-RPC.
//!
//! The two halves of an MCP server that are not tools, and that most servers
//! leave out. They are what makes this something a person can point at a slide
//! with rather than a set of functions a model has to decide to call.
//!
//! The load-bearing test here is `every_resource_this_server_lists_can_be_read`.
//! A resource that appears in a picker and fails to open is worse than one that
//! does not appear at all, and nothing about that failure says which of the two
//! halves — the URI it was listed under, or the reader — is wrong.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use slidx_cli::mcp::{self, Session, Workspace};

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("slidx-mcps-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("slides")).expect("a scratch project");
        Self(path)
    }

    fn slide(&self, name: &str, body: &str) {
        fs::write(self.0.join("slides").join(name), body).expect("write");
    }

    fn built(&self, name: &str, bytes: &[u8]) {
        let dist = self.0.join("dist");
        fs::create_dir_all(&dist).expect("a build directory");
        fs::write(dist.join(name), bytes).expect("write");
    }

    fn path(&self) -> &Path {
        &self.0
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

fn session(scratch: &Scratch) -> Session {
    Session::new(
        Workspace::new(vec![scratch.path().to_path_buf()])
            .with_index(scratch.path().join("no-index.json")),
    )
}

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

fn ask(id: i64, method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

fn result(frames: &[Value], id: i64) -> Value {
    let frame = frames
        .iter()
        .find(|frame| frame["id"] == id)
        .unwrap_or_else(|| panic!("no frame answered {id}: {frames:#?}"));

    assert!(frame["error"].is_null(), "id {id}: {frame}");

    frame["result"].clone()
}

const DECK: &str = "---\ntitle: Making Decks Fast\nevent: SlidxConf\nduration: 20m\n---\n\n\
                    #   Making Decks Fast\n\n\
                    *  the parser\n*  the linter\n\n\
                    The result was [3.2x faster]{#result .accent}.\n";

#[test]
fn the_server_declares_resources_and_prompts_before_a_client_asks_for_either() {
    // A client reads the capabilities to decide what to show a person. A server
    // that served resources and did not declare them has resources nobody sees.
    let scratch = Scratch::new("capabilities");
    scratch.slide("0001.md", DECK);

    let capabilities = result(&talk(&mut session(&scratch), &[hello()]), 1)["capabilities"].clone();

    assert!(capabilities["resources"].is_object(), "{capabilities}");
    assert!(capabilities["prompts"].is_object(), "{capabilities}");
    assert!(capabilities["tools"].is_object(), "{capabilities}");
}

#[test]
fn every_resource_this_server_lists_can_be_read() {
    // A resource that appears in a picker and fails to open is worse than one
    // that does not appear, and nothing about that failure says whether the URI
    // it was listed under or the reader is the wrong half.
    let scratch = Scratch::new("listed");
    scratch.slide("0001.md", DECK);

    let mut held = session(&scratch);
    let listed = result(&talk(&mut held, &[hello(), ask(2, "resources/list", json!({}))]), 2);
    let resources = listed["resources"].as_array().expect("resources").clone();

    assert!(!resources.is_empty());

    for (at, resource) in resources.iter().enumerate() {
        let uri = resource["uri"].as_str().expect("a uri");
        let id = 100 + at as i64;
        let read =
            result(&talk(&mut held, &[ask(id, "resources/read", json!({ "uri": uri }))]), id);
        let contents = read["contents"].as_array().expect("contents");

        assert_eq!(contents.len(), 1, "{uri}");
        assert_eq!(contents[0]["uri"], *uri);
        assert!(contents[0]["text"].is_string() || contents[0]["blob"].is_string(), "{uri}");
    }
}

#[test]
fn a_slides_source_comes_back_as_the_author_wrote_it() {
    // Their spacing and bullet markers are what an edit has to leave alone, so
    // they are what an agent has to be looking at.
    let scratch = Scratch::new("source");
    scratch.slide("0001.md", DECK);

    let uri = format!(
        "slidx://deck/{}/slide/0/source",
        scratch.path().display().to_string().replace('/', "%2F")
    );
    let read = result(
        &talk(&mut session(&scratch), &[hello(), ask(2, "resources/read", json!({ "uri": uri }))]),
        2,
    );

    assert_eq!(read["contents"][0]["mimeType"], "text/markdown");
    let text = read["contents"][0]["text"].as_str().expect("markdown");
    assert!(text.contains("#   Making Decks Fast"), "{text}");
    assert!(text.contains("*  the parser"), "{text}");
}

#[test]
fn a_slide_is_served_as_an_image_so_an_agent_can_look_at_it() {
    // Everything else here is text about a slide. This is a picture of one,
    // which is the difference between reasoning about Markdown and seeing that
    // the title runs to three lines.
    let scratch = Scratch::new("card");
    scratch.slide("0001.md", DECK);

    let uri = format!(
        "slidx://deck/{}/slide/0/card",
        scratch.path().display().to_string().replace('/', "%2F")
    );

    // Not built: the SVG slidx drew, from the deck's own theme tokens.
    let read = result(
        &talk(
            &mut session(&scratch),
            &[hello(), ask(2, "resources/read", json!({ "uri": uri.clone() }))],
        ),
        2,
    );
    assert_eq!(read["contents"][0]["mimeType"], "image/svg+xml");
    assert!(read["contents"][0]["text"].as_str().expect("markup").contains("Making Decks Fast"));

    // Built: the PNG the build rasterised, which is the one almost every client
    // can actually show.
    scratch.built("og-1.png", b"\x89PNG\r\n\x1a\npretend");
    let read = result(
        &talk(&mut session(&scratch), &[hello(), ask(2, "resources/read", json!({ "uri": uri }))]),
        2,
    );

    assert_eq!(read["contents"][0]["mimeType"], "image/png");
    assert!(read["contents"][0]["blob"].as_str().is_some(), "an image is a blob");
    assert!(read["contents"][0]["text"].is_null());
}

#[test]
fn every_template_this_server_offers_can_be_filled_in_and_read() {
    // The templates are how every project the index knows about is reached, so a
    // template that does not fill in is a whole half of the surface nobody can
    // use.
    let scratch = Scratch::new("templates");
    scratch.slide("0001.md", DECK);

    let mut held = session(&scratch);
    let listed =
        result(&talk(&mut held, &[hello(), ask(2, "resources/templates/list", json!({}))]), 2);
    let templates = listed["resourceTemplates"].as_array().expect("templates").clone();

    assert_eq!(templates.len(), 6);
    let project = scratch.path().display().to_string().replace('/', "%2F");

    for (at, template) in templates.iter().enumerate() {
        let uri = template["uriTemplate"]
            .as_str()
            .expect("a template")
            .replace("{project}", &project)
            .replace("{index}", "0");

        let id = 200 + at as i64;
        let read =
            result(&talk(&mut held, &[ask(id, "resources/read", json!({ "uri": uri }))]), id);

        assert_eq!(read["contents"].as_array().expect("contents").len(), 1, "{uri}");
    }
}

#[test]
fn a_resource_outside_the_directories_the_server_serves_is_refused() {
    // A resource is not a way around the authority a tool is held to.
    let scratch = Scratch::new("outside");
    scratch.slide("0001.md", DECK);

    let uri = format!(
        "slidx://deck/{}/model",
        std::env::temp_dir().display().to_string().replace('/', "%2F")
    );
    let frames =
        talk(&mut session(&scratch), &[hello(), ask(2, "resources/read", json!({ "uri": uri }))]);

    assert_eq!(frames[1]["error"]["code"], -32602);
    assert!(frames[1]["error"]["message"].as_str().expect("a reason").contains("outside"));
}

#[test]
fn every_prompt_this_server_lists_can_be_filled_in() {
    let scratch = Scratch::new("prompts");
    scratch.slide("0001.md", DECK);

    let mut held = session(&scratch);
    let listed = result(&talk(&mut held, &[hello(), ask(2, "prompts/list", json!({}))]), 2);
    let prompts = listed["prompts"].as_array().expect("prompts").clone();

    assert_eq!(prompts.len(), 3);

    for (at, prompt) in prompts.iter().enumerate() {
        let name = prompt["name"].as_str().expect("a name");
        let id = 300 + at as i64;

        let filled = result(
            &talk(
                &mut held,
                &[ask(
                    id,
                    "prompts/get",
                    json!({
                        "name": name,
                        "arguments": { "deck": scratch.deck(), "slide": "0" },
                    }),
                )],
            ),
            id,
        );

        let messages = filled["messages"].as_array().expect("messages");
        assert_eq!(messages.len(), 1, "{name}");
        assert_eq!(messages[0]["role"], "user", "{name}");
        assert_eq!(messages[0]["content"]["type"], "text", "{name}");
        assert!(
            messages[0]["content"]["text"].as_str().expect("a text").len() > 200,
            "{name} expanded to almost nothing"
        );
        assert!(filled["description"].as_str().is_some_and(|text| !text.is_empty()), "{name}");
    }
}

#[test]
fn a_prompt_carries_the_deck_rather_than_telling_a_model_to_go_and_read_it() {
    // The thing a prompt can do that a tool description cannot, and the reason
    // serving one is worth anything.
    let scratch = Scratch::new("carries");
    scratch.slide("0001.md", DECK);

    let filled = result(
        &talk(
            &mut session(&scratch),
            &[
                hello(),
                ask(
                    2,
                    "prompts/get",
                    json!({
                        "name": "split_slide",
                        "arguments": { "deck": scratch.deck(), "slide": "0" },
                    }),
                ),
            ],
        ),
        2,
    );

    let text = filled["messages"][0]["content"]["text"].as_str().expect("a text");

    assert!(text.contains("#   Making Decks Fast"), "the author's own bytes: {text}");
    assert!(text.contains("[3.2x faster]{#result .accent}"), "the marks that have to move");
    assert!(text.contains("REUSE THE AUTHOR'S BYTES"), "the rule that matters most");
}

#[test]
fn a_prompt_that_does_not_exist_is_refused_by_name() {
    let scratch = Scratch::new("unknown");
    scratch.slide("0001.md", DECK);

    let frames = talk(
        &mut session(&scratch),
        &[hello(), ask(2, "prompts/get", json!({ "name": "make_it_pop" }))],
    );

    assert_eq!(frames[1]["error"]["code"], -32602);
    assert!(frames[1]["error"]["message"].as_str().expect("a reason").contains("prompts/list"));
}

#[test]
fn the_deck_index_is_served_whatever_the_server_was_pointed_at() {
    // It is the answer to "which talk was that", and the speaker who does not
    // remember the path is exactly the person who needs it.
    let scratch = Scratch::new("index");
    scratch.slide("0001.md", DECK);

    let elsewhere = Scratch::new("index-other");
    elsewhere.slide("0001.md", "---\ntitle: Last year\nevent: VueConf\n---\n\n# Last year\n");

    let index = scratch.path().join("index.json");
    slidx_cli::index::remember(&index, {
        let deck = slidx_core::parse_deck(
            "---\ntitle: Last year\nevent: VueConf\n---\n\n# Last year\n",
            &slidx_core::DeckParseOptions::default(),
        );
        slidx_cli::index::Entry::new(elsewhere.path()).describing(&deck)
    });

    let mut held =
        Session::new(Workspace::new(vec![scratch.path().to_path_buf()]).with_index(index));

    let read = result(
        &talk(&mut held, &[hello(), ask(2, "resources/read", json!({ "uri": "slidx://index" }))]),
        2,
    );
    let body: Value =
        serde_json::from_str(read["contents"][0]["text"].as_str().expect("json")).expect("json");

    let titles: Vec<&str> = body["decks"]
        .as_array()
        .expect("decks")
        .iter()
        .map(|deck| deck["title"].as_str().unwrap_or_default())
        .collect();

    assert!(titles.contains(&"Last year"), "{titles:?}");
}

#[test]
fn reading_a_resource_needs_a_uri_and_says_so() {
    let scratch = Scratch::new("no-uri");
    scratch.slide("0001.md", DECK);

    let frames = talk(&mut session(&scratch), &[hello(), ask(2, "resources/read", json!({}))]);

    assert_eq!(frames[1]["error"]["code"], -32602);
    assert!(frames[1]["error"]["message"].as_str().expect("a reason").contains("uri"));
}
