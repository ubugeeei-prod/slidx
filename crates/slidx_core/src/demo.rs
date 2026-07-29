//! A live demo and the recording that stands in for it.
//!
//! A live demo is the best thing in a talk and the most likely thing to fail.
//! The usual recovery — alt-tab, find the file, apologise — costs the room two
//! minutes and the speaker their place in the talk. Making the fallback part of
//! the deck instead of part of the speaker's memory is what turns that into one
//! keystroke.
//!
//! Declaring it here rather than in the runtime is what lets the linter reach
//! it. A fallback the author forgot is a failure worth catching at the desk,
//! and the desk is where the deck is parsed.
//!
//! The shorthand `demo: <url>` is deliberately allowed even though it produces
//! the exact declaration the linter complains about. An author writing the demo
//! down before they have recorded it is the normal order of events; refusing
//! the shorthand would only mean the demo is not written down at all, and then
//! there is nothing to remind them.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Attribute naming the side currently on screen: `live` or `fallback`.
///
/// The whole switch is one write to this attribute. Both sides are already in
/// the document and CSS decides which is painted, so changing sides creates no
/// element, fetches no file, and has nothing left that can fail at the moment
/// it is needed. It is also why the default belongs in the markup rather than
/// in a script: a deck whose JavaScript never arrives still shows the live
/// demo, which is what the author asked for.
pub const DEMO_ATTRIBUTE: &str = "data-slidx-demo";

/// A demo, as the deck declares it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Demo {
    /// The thing the speaker drives. Expected to be remote — that is what live means.
    pub live: String,
    /// A recording of it working. Absent is what `demo/no-fallback` reports.
    pub fallback: Option<String>,
    /// Frame shown before the recording plays, so the switch is not a black rectangle.
    pub poster: Option<String>,
}

impl Demo {
    /// Whether this demo can survive the live target being gone.
    pub fn has_fallback(&self) -> bool {
        self.fallback.as_ref().is_some_and(|path| !path.trim().is_empty())
    }
}

/// Reads a `demo:` declaration from slide frontmatter.
///
/// A mapping with no `live:` is not a demo. Guessing which key was meant would
/// invent a target the speaker never named, and the deck would open a URL
/// nobody chose in front of an audience.
pub fn parse(matter: &JsonValue) -> Option<Demo> {
    let field = matter.get("demo")?;

    if let Some(live) = field.as_str() {
        return Some(Demo { live: live.trim().to_string(), fallback: None, poster: None });
    }

    Some(Demo {
        live: crate::frontmatter::string(field, "live")?,
        fallback: crate::frontmatter::string(field, "fallback"),
        poster: crate::frontmatter::string(field, "poster"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_demo_declares_a_live_target_and_the_recording_that_replaces_it() {
        let demo = parse(&json!({
            "demo": { "live": "https://app.example.com", "fallback": "./demo.mp4" }
        }))
        .unwrap();

        assert_eq!(demo.live, "https://app.example.com");
        assert_eq!(demo.fallback.as_deref(), Some("./demo.mp4"));
    }

    #[test]
    fn a_slide_with_no_demo_declares_none() {
        assert_eq!(parse(&json!({ "title": "Not a demo" })), None);
    }

    #[test]
    fn a_bare_url_is_a_live_demo_with_no_recording_yet() {
        // The shorthand an author reaches for first, and precisely the state
        // the linter exists to catch: a demo with nothing to fall back to.
        let demo = parse(&json!({ "demo": "https://app.example.com" })).unwrap();

        assert_eq!(demo.live, "https://app.example.com");
        assert_eq!(demo.fallback, None);
    }
}
