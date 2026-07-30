//! The slidx extension for Zed.
//!
//! One job: tell Zed what to run. Everything the author sees — diagnostics,
//! completion, the outline, hover, formatting — comes over the protocol from
//! `slidx lsp`, and this file adds nothing to it.
//!
//! # Why there is no glob here
//!
//! Zed binds a language server to a *language*, so this extension is attached
//! to Markdown and is handed every Markdown file in the project: the README,
//! the changelog, somebody's notes. It has nowhere to put a path rule.
//!
//! That is why the rule is not in any client. `slidx_lsp::deck` decides which
//! URIs are decks, so a file that is not one is never opened and never comes
//! back with a slide diagnostic on it — in Zed, in VS Code, and in Neovim
//! alike. A rule stated in three clients would be three rules; a rule in the
//! server is one, and it has a test.
//!
//! # Why PATH is enough here and is not enough in VS Code
//!
//! Zed resolves `which` through the project's own shell environment, so what
//! it finds is the `slidx` the author's terminal runs — which is what
//! `slidx version use` and a `.slidx-version` pin both act on. A VS Code
//! extension host has no such guarantee and has to look in the install
//! directory as well. Adding that search here would be a second answer to a
//! question this editor already answers correctly.
//!
//! An author who wants a different binary names it in their own settings,
//! which is Zed's own mechanism and not one invented here:
//!
//! ```json
//! { "lsp": { "slidx": { "binary": { "path": "/opt/built/slidx" } } } }
//! ```

use zed_extension_api::{self as zed, settings::LspSettings, Command, LanguageServerId, Result};

/// The binary every install channel puts on the PATH.
const BINARY: &str = "slidx";

/// The subcommand that speaks the protocol.
///
/// slidx ships one binary, so this is a subcommand rather than a `slidx-lsp`
/// beside it: one PATH entry, one release asset, and one version the
/// `.slidx-version` pin applies to.
const SERVER: &str = "lsp";

struct Slidx;

impl zed::Extension for Slidx {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Command> {
        let configured = LspSettings::for_worktree(id.as_ref(), worktree)
            .ok()
            .and_then(|settings| settings.binary)
            .and_then(|binary| binary.path);

        // A path the author typed is taken as given rather than checked. A
        // setting that quietly fell back to something else is how somebody
        // spends an hour debugging the wrong binary.
        let command = match configured {
            Some(path) => path,
            None => worktree.which(BINARY).ok_or_else(nowhere)?,
        };

        Ok(Command { command, args: vec![SERVER.to_string()], env: worktree.shell_env() })
    }
}

/// What an author reads when there is no slidx to start.
///
/// Zed shows this where the language server would have been, so it has to be
/// one sentence that names both the install and the way round it — "slidx not
/// found" is the message that sends somebody to reinstall a binary they have.
fn nowhere() -> String {
    "slidx is not on the PATH Zed sees, so its language server cannot start. \
     Install it with `npm i -g slidx`, or set lsp.slidx.binary.path in your \
     Zed settings to the binary you have — `slidx version current` in a \
     terminal prints where that is."
        .to_string()
}

zed::register_extension!(Slidx);
