//! The half slidx will not do for you.
//!
//! Two of the six destinations need an account. slidx composes what to send
//! them and stops there — deliberately, and this module is where the decision
//! is visible rather than implied.
//!
//! **There is no token store, and there will not be one.** A tool that can post
//! as you is a tool that has to be trusted with a credential, and every
//! credential a tool holds is one that can leak, be committed to a repository,
//! or be used by a dependency nobody audited. The cost of not holding one is a
//! paste; the cost of holding one is somebody else's Speaker Deck account.
//!
//! So the hand-off is: print the payload as fields a person can paste, and name
//! the page they paste it into. `--open` puts that page on screen, and does
//! nothing else — it launches a browser, it does not log in, and it carries
//! nothing with it.

use std::process::Command;

use slidx_publish::{PublishStep, ReadyPayload};

/// A destination and how somebody finishes it by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandOff {
    pub platform: &'static str,
    /// Where an author goes to do this themselves. `None` for the post, which
    /// has no one page — it is text, and it goes wherever they post.
    pub page: Option<&'static str>,
    /// The payload, as lines somebody can read off and paste in.
    pub fields: Vec<(&'static str, String)>,
}

/// Speaker Deck's upload page.
const SPEAKER_DECK_UPLOAD: &str = "https://speakerdeck.com/decks/new";

/// Docswell's own site. The deep link to the upload form is behind a login and
/// changes; the front page does not, and it is one click from either.
const DOCSWELL: &str = "https://www.docswell.com/";

/// What a person still has to do, for a step slidx cannot finish.
pub fn hand_off(step: &PublishStep) -> Option<HandOff> {
    match step.payload()? {
        ReadyPayload::SpeakerDeck(upload) => Some(HandOff {
            platform: "Speaker Deck",
            page: Some(SPEAKER_DECK_UPLOAD),
            fields: vec![
                ("file", upload.pdf.clone()),
                ("title", upload.title.clone()),
                ("description", upload.description.clone()),
                ("slug", upload.slug.clone()),
                ("tags", upload.tags.join(", ")),
                ("date", upload.date.clone().unwrap_or_default()),
            ],
        }),
        ReadyPayload::Docswell(upload) => Some(HandOff {
            platform: "Docswell",
            page: Some(DOCSWELL),
            fields: vec![
                ("file", upload.file.clone()),
                ("title", upload.title.clone()),
                ("overview", upload.overview.clone()),
                ("path", upload.path.clone()),
                ("tags", upload.tags.join(", ")),
                ("presented at", upload.presented_at.clone().unwrap_or_default()),
            ],
        }),
        ReadyPayload::Social(post) => Some(HandOff {
            platform: "Post",
            // No page: a post goes wherever the author posts, and guessing a
            // network would be slidx choosing one on their behalf.
            page: None,
            fields: vec![
                ("text", post.text.clone()),
                ("length", format!("{}/{} characters", post.length, post.limit)),
                ("image", post.image.clone().unwrap_or_default()),
            ],
        }),
        ReadyPayload::Blog(_) | ReadyPayload::Resources(_) | ReadyPayload::Archive(_) => None,
    }
}

impl HandOff {
    /// The fields worth printing: the ones that have a value.
    ///
    /// An empty `date` is a line the author has to read and discard. The plan
    /// already said what is missing.
    pub fn written(&self) -> impl Iterator<Item = (&'static str, &str)> {
        self.fields
            .iter()
            .filter(|(_, value)| !value.is_empty())
            .map(|(name, value)| (*name, value.as_str()))
    }
}

/// Puts a page on screen.
///
/// Best-effort, and never fatal: a machine with no browser, a headless CI
/// runner, a sandbox that refuses to spawn — none of those mean the publish
/// failed, because the page was printed as well as opened. The whole point of
/// printing it is that opening it is a convenience.
pub fn open(url: &str) -> bool {
    let (command, first) = if cfg!(target_os = "macos") {
        ("open", None)
    } else if cfg!(target_os = "windows") {
        // `start` is a shell builtin rather than a program, and its first
        // argument is taken as a window title — hence the empty one.
        ("cmd", Some(vec!["/C", "start", ""]))
    } else {
        ("xdg-open", None)
    };

    let mut process = Command::new(command);
    if let Some(arguments) = first {
        process.args(arguments);
    }

    process.arg(url).status().is_ok_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_publish::{
        plan_publish, Artifact, ArtifactKind, DeckMetadata, PlanOptions, PublishTarget,
    };

    fn steps(targets: Vec<PublishTarget>) -> Vec<PublishStep> {
        plan_publish(&PlanOptions {
            meta: DeckMetadata {
                title: Some("Zero-JavaScript Slides".into()),
                description: Some("Why a deck should be plain HTML.".into()),
                event: Some("SlidxConf 2026".into()),
                date: Some("2026-07-29".into()),
                hashtag: Some("slidxconf".into()),
                url: Some("https://slidx.dev/talks/zero-js".into()),
                tags: Some(vec!["rust".into()]),
                ..DeckMetadata::default()
            },
            artifacts: vec![Artifact {
                kind: ArtifactKind::Pdf,
                path: "dist/deck.pdf".into(),
                bytes: Some(1024),
            }],
            targets: Some(targets),
            ..PlanOptions::default()
        })
        .steps
    }

    fn one(target: PublishTarget) -> HandOff {
        hand_off(&steps(vec![target])[0]).expect("a hand-off")
    }

    #[test]
    fn a_destination_that_needs_an_account_is_handed_over_with_its_fields_named() {
        // Field names are the platform's, so a person reading the report is
        // reading the labels on the form they are about to fill in.
        let speaker_deck = one(PublishTarget::Speakerdeck);

        assert_eq!(speaker_deck.platform, "Speaker Deck");
        assert!(speaker_deck
            .written()
            .any(|(name, value)| name == "file" && value == "dist/deck.pdf"));
        assert!(speaker_deck.written().any(|(name, _)| name == "slug"));
    }

    #[test]
    fn the_two_slide_hosts_are_handed_over_with_their_own_field_names() {
        // The same deck, and deliberately not the same form. Sharing one set of
        // labels would have somebody hunting for "description" on a page that
        // calls it an overview.
        let docswell = one(PublishTarget::Docswell);

        assert!(docswell.written().any(|(name, _)| name == "overview"));
        assert!(!docswell.written().any(|(name, _)| name == "description"));
    }

    #[test]
    fn a_post_is_handed_over_without_a_page_because_it_has_no_one_destination() {
        // Naming a network would be slidx choosing one on the author's behalf.
        let post = one(PublishTarget::Social);

        assert_eq!(post.page, None);
        assert!(post.written().any(|(name, value)| name == "text" && value.contains("#slidxconf")));
    }

    #[test]
    fn a_page_slidx_writes_itself_is_not_handed_over() {
        for target in [PublishTarget::Resources, PublishTarget::Archive] {
            assert_eq!(hand_off(&steps(vec![target])[0]), None);
        }
    }

    #[test]
    fn a_field_the_deck_has_no_value_for_is_left_out_rather_than_printed_empty() {
        // A line the author has to read and discard. The plan already said what
        // is missing.
        let post = one(PublishTarget::Social);

        assert!(!post.written().any(|(name, _)| name == "image"));
    }

    #[test]
    fn the_upload_pages_are_pages_rather_than_endpoints() {
        // There is no HTTP client anywhere under this command, and these are
        // addresses for a browser. A path that looked like an API would be an
        // invitation to add one.
        for url in [SPEAKER_DECK_UPLOAD, DOCSWELL] {
            assert!(url.starts_with("https://"), "{url}");
            assert!(!url.contains("/api/"), "{url}");
        }
    }
}
