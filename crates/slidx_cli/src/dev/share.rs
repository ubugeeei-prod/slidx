//! The share link, and every decision that makes it safe to print.
//!
//! `slidx dev --crdt` puts a dev server that can write the author's files onto a
//! network. That is a security decision, so it is one module rather than a flag
//! read in four places, and the four rules it keeps are the ones a convenient
//! default gets wrong.
//!
//! **The local network, not the internet.** A URL a co-presenter opens on the
//! same Wi-Fi is what a conference needs, and it involves no third party at all.
//! slidx offers **no tunnel**, and there is no flag that adds one. A public URL
//! to an unannounced talk, served by a process that can write the author's files,
//! through a third party that then sees the traffic, is a different feature with
//! a different blast radius — and the version of it that is one flag away from
//! `--crdt` is the version somebody reaches for by accident. `slidx dev --help`
//! says so, because a decision nobody can read is one somebody will file as a
//! bug.
//!
//! **Bound beyond localhost only when sharing was asked for.** Without `--crdt`,
//! Vite is started with no `--host` and the deck is reachable from this machine
//! and nowhere else, exactly as before.
//!
//! **The secret is in the fragment.** After the `#`, never in the query. A
//! fragment is not sent with the request, so it reaches no access log, no
//! referrer header and no proxy record. The shape is `pairingUrl`'s from
//! `packages/runtime/src/remote.ts`, and the *reader* on the other side is that
//! file's own `readPairing` — so a URL slidx prints and a URL the editor accepts
//! cannot drift apart without a test failing.
//!
//! **Read-only unless editing was granted separately.** Two secrets, not one
//! secret and a flag. A viewer holding the read link cannot reach the edit route
//! because they were never given the bytes that open it, which is a property
//! rather than a policy.

pub mod address;
pub mod qr;
pub mod secret;

use std::net::IpAddr;

use crate::report;
use crate::style::{Ink, Style};

use secret::{NoRandomness, SECRET_BYTES, SESSION_BYTES};

/// The fragment key the secret travels under.
///
/// `FRAGMENT_KEY` in `packages/runtime/src/remote.ts`, so other fragment
/// parameters can coexist with it. Changing it here alone would print links the
/// editor reads nothing out of.
const FRAGMENT_KEY: &str = "s";

/// The editor's route, which is what a share link points at.
const EDITOR_ROUTE: &str = super::EDITOR_ROUTE;

/// The environment variables the dev server reads the secrets out of.
///
/// The environment rather than the command line, because an argument list is
/// readable by every process on the machine and an environment is not. Named the
/// same as `SHARE_VARIABLE` and `SHARE_EDIT_VARIABLE` in
/// `packages/vite-plugin/src/share.ts`, which is the only reader.
pub const SHARE_VARIABLE: &str = "SLIDX_SHARE";
pub const SHARE_EDIT_VARIABLE: &str = "SLIDX_SHARE_EDIT";
/// Public LAN origin the plugin uses to rebuild the printed links for the author.
pub const SHARE_ORIGIN_VARIABLE: &str = "SLIDX_SHARE_ORIGIN";

/// One shared session's capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Share {
    /// Names the session. Not secret, and it appears in every link.
    pub session: String,
    /// Opens the deck for reading.
    pub read: String,
    /// Opens it for editing. `None` unless `--allow-edit` was given.
    pub edit: Option<String>,
}

impl Share {
    /// Mints a session, and an edit secret only if one was asked for.
    ///
    /// The edit secret is not minted and withheld — it does not exist. A secret
    /// that was generated and then not printed is still a secret in a process's
    /// environment, and the honest shape of "nobody may edit" is that there is
    /// nothing to present.
    pub fn mint(allow_edit: bool) -> Result<Self, NoRandomness> {
        Ok(Self {
            session: secret::token(SESSION_BYTES)?,
            read: secret::token(SECRET_BYTES)?,
            edit: if allow_edit { Some(secret::token(SECRET_BYTES)?) } else { None },
        })
    }

    /// The credential for one secret, as the environment and the fragment spell it.
    pub fn credential(&self, secret: &str) -> String {
        format!("{}.{secret}", self.session)
    }

    /// The link to hand somebody, with the secret after the `#`.
    pub fn link(&self, origin: &str, secret: &str) -> String {
        format!("{origin}{EDITOR_ROUTE}#{FRAGMENT_KEY}={}", self.credential(secret))
    }

    /// The variables to start the dev server with.
    pub fn environment(&self) -> Vec<(&'static str, String)> {
        let mut variables = vec![(SHARE_VARIABLE, self.credential(&self.read))];

        if let Some(edit) = &self.edit {
            variables.push((SHARE_EDIT_VARIABLE, self.credential(edit)));
        }

        variables
    }
}

/// What is printed under the ready line when a deck is shared.
///
/// The read link, its code, and — only when editing was granted — the second
/// link and its own code. Then the two sentences somebody needs before they read
/// the URL out to a room.
pub fn block(share: &Share, address: IpAddr, port: u16, indent: usize, style: &Style) -> String {
    let origin = address::origin(address, port);
    let mut text = String::new();

    text.push('\n');
    text.push_str(&report::flowed("read only — anyone with this link", indent, Ink::Faint, style));
    text.push_str(&link(&share.link(&origin, &share.read), indent, style));

    if let Some(edit) = &share.edit {
        text.push('\n');
        text.push_str(&report::flowed(
            "can edit — this one rewrites your slide files",
            indent,
            Ink::Warn,
            style,
        ));
        text.push_str(&link(&share.link(&origin, edit), indent, style));
    }

    text.push('\n');
    text.push_str(&report::flowed(caution(share), indent, Ink::Faint, style));

    text
}

/// One link, and its code underneath.
fn link(url: &str, indent: usize, style: &Style) -> String {
    let mut text = report::flowed(url, indent, Ink::Strong, style);
    let code = qr::block(url, indent, style);

    if !code.is_empty() {
        text.push('\n');
        text.push_str(&code);
    }

    text
}

/// The sentence that has to be read before the URL is.
fn caution(share: &Share) -> &'static str {
    match share.edit {
        Some(_) => {
            "Both links work for anyone on this network who has them, and the second one \
                    can change your deck. They stop working when this dev server does."
        }
        None => {
            "This link works for anyone on this network who has it. It cannot change your \
                 deck, and it stops working when this dev server does."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn share() -> Share {
        Share {
            session: "0123456789abcdef".into(),
            read: "00112233445566778899aabbccddeeff".into(),
            edit: Some("ffeeddccbbaa99887766554433221100".into()),
        }
    }

    fn here() -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 42))
    }

    #[test]
    fn a_link_carries_the_secret_after_the_hash_and_nowhere_else() {
        // The single character this whole module exists for. A query parameter
        // reaches an access log, and a log holding this value is enough to join.
        let url = share().link("http://192.168.1.42:5173", &share().read);

        assert_eq!(
            url,
            "http://192.168.1.42:5173/__slidx/#s=0123456789abcdef.00112233445566778899aabbccddeeff"
        );
        assert!(!url.contains('?'), "{url}");
        assert!(url.split('#').next().is_some_and(|before| !before.contains(&share().read)));
    }

    #[test]
    fn a_links_fragment_is_shaped_the_way_the_phone_remote_reads_one() {
        // `readPairing` in packages/runtime/src/remote.ts is the reader, and it
        // wants `s=<session>.<secret>` in lowercase hex. This is the pin on the
        // Rust side; `test/share.test.ts` is the pin on the other.
        let credential = share().credential(&share().read);

        assert_eq!(credential, "0123456789abcdef.00112233445566778899aabbccddeeff");
        assert!(credential.split('.').all(|part| part.chars().all(|c| c.is_ascii_hexdigit())));
    }

    #[test]
    fn a_session_without_edit_access_has_no_edit_secret_at_all() {
        // Not minted and withheld — absent. A secret in a process's environment
        // is a secret whether or not it was printed.
        let read_only = Share::mint(false).expect("randomness");

        assert!(read_only.edit.is_none());
        assert_eq!(read_only.environment().len(), 1);
        assert_eq!(read_only.environment()[0].0, SHARE_VARIABLE);
    }

    #[test]
    fn granting_edit_access_adds_a_second_secret_rather_than_changing_the_first() {
        let both = Share::mint(true).expect("randomness");

        assert_ne!(both.edit.as_deref(), Some(both.read.as_str()));
        assert_eq!(both.environment().len(), 2);
    }

    #[test]
    fn the_variables_are_the_ones_the_plugin_reads() {
        // Named in packages/vite-plugin/src/share.ts, which is the only reader.
        assert_eq!(SHARE_VARIABLE, "SLIDX_SHARE");
        assert_eq!(SHARE_EDIT_VARIABLE, "SLIDX_SHARE_EDIT");
    }

    #[test]
    fn a_read_only_share_says_the_link_cannot_change_the_deck() {
        let text = block(&Share { edit: None, ..share() }, here(), 5173, 2, &Style::plain());

        assert!(text.contains("read only"), "{text}");
        assert!(text.contains("cannot change your"), "{text}");
        assert!(!text.contains("can edit"), "{text}");
    }

    #[test]
    fn a_share_that_grants_editing_warns_about_the_link_that_does_it() {
        // Somebody about to read a URL out to a room needs to know which of the
        // two they are reading.
        let text = block(&share(), here(), 5173, 2, &Style::plain());

        assert!(text.contains("can edit"), "{text}");
        assert!(text.contains("rewrites your slide files"), "{text}");
        assert!(text.contains(&share().read), "{text}");
        assert!(text.contains(share().edit.as_deref().unwrap()), "{text}");
    }

    #[test]
    fn a_share_block_says_the_links_end_with_the_dev_server() {
        // The one reassurance that is actually true: nothing outlives the
        // process, because nothing else is holding the secret.
        assert!(
            block(&share(), here(), 5173, 2, &Style::plain()).contains("when this dev server does")
        );
    }

    #[test]
    fn a_share_block_carries_no_escape_sequences_when_colour_is_off() {
        assert!(!block(&share(), here(), 5173, 2, &Style::plain()).contains('\u{1b}'));
    }

    #[test]
    fn a_coloured_share_block_draws_the_code_under_the_link() {
        let text = block(&share(), here(), 5173, 2, &Style::colored());

        assert!(text.contains('\u{2588}') || text.contains('\u{2580}'), "no code drawn");
    }
}
