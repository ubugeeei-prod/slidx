//! What an agent can read without asking for anything to happen.
//!
//! Resources rather than tools because these are *data a client can attach*. A
//! model that had to call a tool to see a slide has to decide to; a resource can
//! be put in front of it by the person driving, which is how somebody says "this
//! slide" without pasting it.
//!
//! ## What is listed and what is templated
//!
//! The concrete list is the deck index and the directories this server was
//! pointed at. Every project the index knows about is reachable through the
//! templates instead, because a speaker with two hundred talks would otherwise
//! get six hundred entries in a picker and find nothing in it.
//!
//! ## The visual one
//!
//! `card` is an image, so an agent can *look* at the slide it is editing rather
//! than infer it from Markdown. slidx draws the card as SVG, from the same theme
//! tokens as the deck, and the build rasterises it with the browser that is
//! already there for the PDF. So this serves the PNG the build wrote when there
//! is one, and the SVG when there is not — and says which, rather than pretending
//! a card exists that nobody converted.
//!
//! A card is not a screenshot of the slide, and the description says so. A slide
//! is designed to be read from twelve metres and a card at four hundred pixels
//! wide, so it carries fewer words, larger.

pub mod card;
pub mod deck;

use serde_json::{json, Value};

use super::uri::{self, DeckView, Resource, SlideView};
use super::workspace::Workspace;

/// The resources a client is told about by name.
///
/// The index, and the three deck-shaped views for each directory this server was
/// started in or pointed at.
pub fn list(workspace: &Workspace) -> Vec<Value> {
    let mut listed = vec![json!({
        "uri": uri::INDEX,
        "name": "deck_index",
        "title": "Every deck this machine has seen",
        "description": "\
    The projects slidx has run a command on, most recently touched first, with the \
    title, event and date each deck says about itself. The index fills itself, so \
    this is the answer to \"which talk was that\" without anybody having registered \
    anything.",
        "mimeType": "application/json",
    })];

    for root in workspace.roots() {
        let name = root.file_name().map(|name| name.to_string_lossy().into_owned());
        let name = name.unwrap_or_else(|| root.display().to_string());

        for (view, title, description, mime) in DECK_VIEWS {
            listed.push(json!({
                "uri": uri::deck(root, *view),
                "name": format!("{name}_{}", view_token(*view)),
                "title": format!("{title}: {name}"),
                "description": *description,
                "mimeType": *mime,
            }));
        }
    }

    listed
}

/// The shapes a whole deck is served in, described once.
const DECK_VIEWS: &[(DeckView, &str, &str, &str)] = &[
    (
        DeckView::Model,
        "The parsed deck",
        "\
Everything slidx knows about the deck: its metadata, every slide with its title, \
body, notes, marks, budget and layout, and the diagnostics the parse produced. \
This is the same model the build, the editor and the presenter view all consume, \
so what it says is what will be on the wall.",
        "application/json",
    ),
    (
        DeckView::Diagnostics,
        "What a room will do to this deck",
        "\
Parse diagnostics and lint findings, worst first then in deck order. Contrast \
through a model of projector washout, font size by the angular size at the back \
row, offline assets, heading order, animation cost, and time budgets against the \
slot. Every finding carries a concrete next action.",
        "application/json",
    ),
    (
        DeckView::Timeline,
        "The compiled steps",
        "\
Every stop of every slide, as a complete state rather than a delta. This is what \
advancing, going back, deep-linking to `?step=7` and printing all index into, so \
it is the answer to \"how many clicks is this deck\" and \"what does the audience \
see at stop 3\".",
        "application/json",
    ),
];

fn view_token(view: DeckView) -> &'static str {
    match view {
        DeckView::Model => "model",
        DeckView::Diagnostics => "diagnostics",
        DeckView::Timeline => "timeline",
    }
}

/// The templates for everything not worth listing one by one.
pub fn templates() -> Vec<Value> {
    vec![
        template(
            "slidx://deck/{project}/model",
            "deck_model",
            "A deck's parsed model",
            "\
`project` is the project directory, percent-encoded. Any project this server was \
pointed at, or that the deck index knows about.",
            "application/json",
        ),
        template(
            "slidx://deck/{project}/diagnostics",
            "deck_diagnostics",
            "A deck's diagnostics and lint findings",
            "Parse problems and what a room will do to the deck, worst first.",
            "application/json",
        ),
        template(
            "slidx://deck/{project}/timeline",
            "deck_timeline",
            "A deck's compiled step timeline",
            "Every stop of every slide, as a complete state.",
            "application/json",
        ),
        template(
            "slidx://deck/{project}/slide/{index}/source",
            "slide_source",
            "One slide's Markdown",
            "\
Exactly as the author wrote it, their spacing and bullet markers included. \
`index` counts from zero, the same as the editing tools — and deliberately not \
the same as the linter's report, which says \"slide 2\" because nobody counts \
slides from zero out loud.",
            "text/markdown",
        ),
        template(
            "slidx://deck/{project}/slide/{index}/html",
            "slide_html",
            "One slide's rendered HTML",
            "\
What the browser gets, step anchors included. Useful for seeing what a mark or a \
step actually compiled to; not useful for judging appearance, which is the \
theme's and the room's business.",
            "text/html",
        ),
        template(
            "slidx://deck/{project}/slide/{index}/card",
            "slide_card",
            "One slide's social card, as an image",
            "\
Look at this when you need to see a slide rather than read it. A PNG once the \
deck has been built, and the SVG slidx drew otherwise.

It is NOT a screenshot of the slide. A slide is designed to be read from twelve \
metres and a card at four hundred pixels wide in a crowded feed, so the card \
carries fewer words, larger, in the deck's own theme.",
            "image/png",
        ),
    ]
}

fn template(uri: &str, name: &str, title: &str, description: &str, mime: &str) -> Value {
    json!({
        "uriTemplate": uri,
        "name": name,
        "title": title,
        "description": description,
        "mimeType": mime,
    })
}

/// The contents of one resource.
///
/// The refusal is a value rather than a protocol error, for the same reason a
/// failing tool is: a client can put a stale URI in front of a model, and the
/// model is the one that has to notice and ask for something else.
pub fn read(workspace: &Workspace, uri: &str) -> Result<Vec<Value>, String> {
    let Some(resource) = uri::parse(uri) else {
        return Err(format!(
            "`{uri}` is not a resource this server serves. Ask `resources/templates/list` for \
             the shapes there are."
        ));
    };

    match resource {
        Resource::Index => Ok(vec![text(uri, "application/json", deck::index(workspace)?)]),
        Resource::Deck { project, view } => deck::view(workspace, uri, &project, view),
        Resource::Slide { project, index, view } => match view {
            SlideView::Card => card::read(workspace, uri, &project, index),
            _ => deck::slide(workspace, uri, &project, index, view),
        },
    }
}

/// One resource's contents, as text.
pub fn text(uri: &str, mime: &str, body: String) -> Value {
    json!({ "uri": uri, "mimeType": mime, "text": body })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace() -> Workspace {
        Workspace::new(vec![PathBuf::from("/talks/vueconf")])
            .with_index(PathBuf::from("/nowhere/index.json"))
    }

    #[test]
    fn every_listed_resource_names_a_uri_this_server_can_parse() {
        // A resource a client can list and never read is the failure the URI
        // module exists to prevent, and this is where the two meet.
        for listed in list(&workspace()) {
            let uri = listed["uri"].as_str().expect("a uri");

            assert!(uri::parse(uri).is_some(), "{uri} is listed and unparseable");
            assert!(listed["description"].as_str().is_some_and(|text| text.len() > 40), "{listed}");
            assert!(listed["mimeType"].as_str().is_some(), "{listed}");
        }
    }

    #[test]
    fn the_index_is_listed_whatever_the_server_was_pointed_at() {
        // It is the answer to "which talk was that", and a speaker who does not
        // know the path is exactly the person who needs it.
        let listed = list(&workspace());

        assert_eq!(listed[0]["uri"], uri::INDEX);
    }

    #[test]
    fn a_root_is_listed_in_every_shape_a_deck_is_served_in() {
        let listed = list(&workspace());

        assert_eq!(listed.len(), 1 + DECK_VIEWS.len());
        for view in ["model", "diagnostics", "timeline"] {
            assert!(
                listed
                    .iter()
                    .any(|entry| entry["uri"].as_str().unwrap_or_default().ends_with(view)),
                "no {view} was listed"
            );
        }
    }

    #[test]
    fn indexed_projects_are_templated_rather_than_listed_one_by_one() {
        // A speaker with two hundred talks would otherwise get six hundred
        // entries in a picker and find nothing in it.
        assert_eq!(list(&workspace()).len(), 4, "the index and one root's three views");
        assert_eq!(templates().len(), 6);
    }

    #[test]
    fn every_template_is_a_uri_a_client_can_fill_in() {
        for template in templates() {
            let uri = template["uriTemplate"].as_str().expect("a template");

            assert!(uri.starts_with("slidx://"), "{uri}");
            assert!(uri.contains("{project}"), "{uri}");
            assert!(template["description"].as_str().is_some_and(|text| !text.is_empty()));
        }
    }

    #[test]
    fn the_card_template_says_it_is_not_a_screenshot() {
        // A model that believed otherwise would judge a slide's layout from it
        // and be confidently wrong.
        let card = templates().into_iter().find(|t| t["name"] == "slide_card").expect("a card");

        assert!(card["description"].as_str().expect("a text").contains("NOT a screenshot"));
    }

    #[test]
    fn a_uri_this_server_does_not_serve_is_answered_rather_than_thrown() {
        let refusal = read(&workspace(), "https://example.com/deck").expect_err("not ours");

        assert!(refusal.contains("templates/list"), "{refusal}");
    }
}
