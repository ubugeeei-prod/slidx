//! The deck-shaped resources: the index, a model, a slide's source and HTML.
//!
//! Everything here is a serialisation of something slidx already computed. The
//! model is [`slidx_core`]'s, the diagnostics are the linter's, the timeline is
//! the step compiler's, and the HTML is [`slidx_render`]'s — the same one the
//! build emits, step anchors included.
//!
//! Nothing is summarised or reshaped on the way out. A resource that described
//! a deck in its own words would be a fourth opinion about what a deck is, and
//! the whole workspace is arranged so there are not four.

use std::path::Path;

use serde_json::json;
use slidx_render::{render_markdown, MarkdownOptions};

use crate::index::Index;
use crate::mcp::resource::text;
use crate::mcp::uri::{DeckView, SlideView};
use crate::mcp::workspace::Workspace;

/// Every deck this machine has seen, most recently touched first.
pub fn index(workspace: &Workspace) -> Result<String, String> {
    let index = Index::load(&workspace.index_path());

    let entries: Vec<_> = index
        .live()
        .map(|entry| {
            json!({
                "path": entry.path.display().to_string(),
                "label": entry.label(),
                "title": entry.title,
                "event": entry.event,
                "date": entry.date,
                "occasion": entry.occasion(),
                "lastSeen": entry.last_seen,
            })
        })
        .collect();

    serde_json::to_string_pretty(&json!({ "decks": entries }))
        .map_err(|error| format!("Could not serialise the deck index: {error}"))
}

/// One of a whole deck's shapes.
pub fn view(
    workspace: &Workspace,
    uri: &str,
    project: &Path,
    view: DeckView,
) -> Result<Vec<serde_json::Value>, String> {
    let reading = workspace.read_deck(&project.display().to_string(), None)?;

    let body = match view {
        DeckView::Model => serde_json::to_string_pretty(&reading.deck),
        DeckView::Diagnostics => {
            let found = crate::lint::findings(&reading.deck, None, &Default::default());
            serde_json::to_string_pretty(&json!({
                "deck": reading.path.display().to_string(),
                "blocking": found.iter().filter(|d| d.is_blocking()).count(),
                "diagnostics": found,
            }))
        }
        DeckView::Timeline => serde_json::to_string_pretty(&json!({
            "stops": reading.deck.stop_count(),
            "slides": reading
                .deck
                .slides
                .iter()
                .map(|slide| json!({
                    "index": slide.index,
                    "id": slide.id,
                    "title": slide.display_title(),
                    "stops": slide.stop_count(),
                    "timeline": slide.timeline,
                }))
                .collect::<Vec<_>>(),
        })),
    };

    let body = body.map_err(|error| format!("Could not serialise the deck: {error}"))?;

    Ok(vec![text(uri, "application/json", body)])
}

/// One slide, as source or as HTML.
pub fn slide(
    workspace: &Workspace,
    uri: &str,
    project: &Path,
    index: usize,
    view: SlideView,
) -> Result<Vec<serde_json::Value>, String> {
    let reading = workspace.read_deck(&project.display().to_string(), None)?;
    let slide = reading.deck.slides.get(index).ok_or_else(|| missing(index, &reading))?;

    match view {
        SlideView::Source => {
            // The author's own bytes, not the parsed content: their spacing and
            // bullet markers are what an edit has to leave alone, so they are
            // what an agent should be looking at.
            let options = slidx_core::DeckParseOptions {
                separator: reading.separator.clone(),
                ..Default::default()
            };
            let spans = slidx_edit::slide_spans(&reading.source, &options);
            let body = spans
                .get(index)
                .map(|span| span.content.slice(&reading.source).to_string())
                .unwrap_or_default();

            Ok(vec![text(uri, "text/markdown", body)])
        }
        SlideView::Html => Ok(vec![text(
            uri,
            "text/html",
            render_markdown(&slide.content, &MarkdownOptions::default()),
        )]),
        // Handled by the module that knows about images.
        SlideView::Card => Err("a card is not text".to_string()),
    }
}

/// Says how many slides there are, so the next attempt can be right.
pub fn missing(index: usize, reading: &crate::mcp::workspace::Reading) -> String {
    format!(
        "There is no slide {index} in {}: it has {} slide(s), numbered from zero.",
        reading.label,
        reading.deck.slides.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::uri;
    use std::fs;
    use std::path::PathBuf;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-res-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(path.join("slides")).expect("scratch");
            Self(path)
        }

        fn slide(&self, name: &str, body: &str) {
            fs::write(self.0.join("slides").join(name), body).expect("write");
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn workspace(&self) -> Workspace {
            Workspace::new(vec![self.0.clone()]).with_index(self.0.join("no-index.json"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn body(contents: Vec<serde_json::Value>) -> String {
        contents[0]["text"].as_str().expect("a text").to_string()
    }

    #[test]
    fn a_decks_model_is_the_one_the_build_consumes() {
        let scratch = Scratch::new("model");
        scratch.slide("0001.md", "---\ntitle: A talk\n---\n\n# One\n");

        let uri = uri::deck(scratch.path(), DeckView::Model);
        let read = view(&scratch.workspace(), &uri, scratch.path(), DeckView::Model).expect("read");
        let model: serde_json::Value = serde_json::from_str(&body(read)).expect("json");

        assert_eq!(model["meta"]["title"], "A talk");
        assert_eq!(model["slides"][0]["title"], "One");
    }

    #[test]
    fn a_slides_source_is_the_authors_own_bytes_rather_than_the_parsed_content() {
        // Their spacing and bullet markers are what an edit has to leave alone,
        // so they are what an agent should be looking at.
        let scratch = Scratch::new("source");
        scratch.slide("0001.md", "#   One\n\n*  a\n*  b\n");

        let uri = uri::slide(scratch.path(), 0, SlideView::Source);
        let read =
            slide(&scratch.workspace(), &uri, scratch.path(), 0, SlideView::Source).expect("read");

        assert_eq!(body(read), "#   One\n\n*  a\n*  b");
    }

    #[test]
    fn a_slides_html_carries_the_step_anchors_the_runtime_reads() {
        // The whole reason to look at the HTML: seeing what a step compiled to.
        let scratch = Scratch::new("html");
        scratch.slide("0001.md", "# One\n\n- a <!-- step -->\n");

        let uri = uri::slide(scratch.path(), 0, SlideView::Html);
        let read =
            slide(&scratch.workspace(), &uri, scratch.path(), 0, SlideView::Html).expect("read");

        assert!(body(read).contains("data-slidx-step="));
    }

    #[test]
    fn the_diagnostics_are_the_linters_own_findings() {
        let scratch = Scratch::new("diagnostics");
        scratch.slide("0001.md", "# One\n\n![a](https://cdn.example.com/a.png)\n");

        let uri = uri::deck(scratch.path(), DeckView::Diagnostics);
        let read =
            view(&scratch.workspace(), &uri, scratch.path(), DeckView::Diagnostics).expect("read");
        let found: serde_json::Value = serde_json::from_str(&body(read)).expect("json");

        assert_eq!(found["blocking"], 1);
        assert_eq!(found["diagnostics"][0]["code"], "offline/remote-asset");
    }

    #[test]
    fn the_timeline_counts_the_stops_a_speaker_will_click_through() {
        let scratch = Scratch::new("timeline");
        scratch.slide("0001.md", "# One\n\n- a <!-- step -->\n- b <!-- step -->\n");

        let uri = uri::deck(scratch.path(), DeckView::Timeline);
        let read =
            view(&scratch.workspace(), &uri, scratch.path(), DeckView::Timeline).expect("read");
        let timeline: serde_json::Value = serde_json::from_str(&body(read)).expect("json");

        assert_eq!(timeline["stops"], 3, "the slide itself, and two reveals");
        assert_eq!(timeline["slides"][0]["title"], "One");
    }

    #[test]
    fn a_slide_that_is_not_there_says_how_many_there_are() {
        // So the next attempt can be right rather than another guess.
        let scratch = Scratch::new("missing");
        scratch.slide("0001.md", "# One\n");

        let uri = uri::slide(scratch.path(), 9, SlideView::Source);
        let refusal = slide(&scratch.workspace(), &uri, scratch.path(), 9, SlideView::Source)
            .expect_err("no such slide");

        assert!(refusal.contains("1 slide(s)"), "{refusal}");
        assert!(refusal.contains("numbered from zero"), "{refusal}");
    }

    #[test]
    fn a_project_outside_the_roots_is_refused_the_same_way_a_tool_is() {
        // A resource is not a way around the authority a tool is held to.
        let scratch = Scratch::new("outside");
        let above = std::env::temp_dir();

        let uri = uri::deck(&above, DeckView::Model);
        let refusal =
            view(&scratch.workspace(), &uri, &above, DeckView::Model).expect_err("outside");

        assert!(refusal.contains("outside"), "{refusal}");
    }
}
