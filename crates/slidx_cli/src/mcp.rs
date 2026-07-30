//! `slidx mcp` — slidx, served to an agent.
//!
//! ## Why this exists rather than letting an agent edit the Markdown
//!
//! An agent that changes a deck by rewriting the file reformats the parts it did
//! not mean to touch. The author's blank lines are regularised, their `*` bullets
//! become `-`, their hand-wrapped paragraph becomes one long line — every one of
//! those invisible on a slide and enormous in the diff. That is exactly the
//! failure [`slidx_edit`] exists to prevent, and the reason the visual editor's
//! round trip holds: an edit is a byte-range splice into the source the author
//! saved, and the bytes it does not name are never read and cannot change.
//!
//! So this server gives an agent slidx's own operations rather than a file
//! writer. Not as a convenience — as a constraint. An agent working through it is
//! *structurally* incapable of reflowing a paragraph or reordering frontmatter,
//! because there is no call that takes raw file content.
//!
//! ## Every mutation hands back its own inverse
//!
//! [`slidx_edit::Edit`] is already a value that knows how to reverse itself, so
//! this is nearly free — and no other editing surface an agent has can offer it.
//! An agent told its third change was wrong walks back three calls, byte for
//! byte, rather than reconstructing a file from memory. See [`history`].
//!
//! ## Read-only by default, and stdio only
//!
//! Writes are behind `--write`, and a mutating tool is neither listed nor
//! runnable without it. A server that would rewrite a conference talk because
//! something in the deck told it to is a liability, and a deck's own slides,
//! notes and code fences are untrusted input that an agent reads on the author's
//! behalf. [`instructions`] says so to the client, and no resource's content
//! changes what the server is willing to do.
//!
//! There is no listener, no port, and no outbound request. The transport is
//! standard input and standard output, and the only thing this process can reach
//! is the filesystem under the directories it was given. Serving a deck to a
//! browser is `slidx preview --web`, which a person runs.
//!
//! ## Not one line of anything else on standard output
//!
//! Standard output is the wire. One stray `println!` desynchronises the client
//! for the rest of the session, so [`run`] returns an empty [`Outcome`] and
//! anything worth saying goes to standard error.

pub mod content;
pub mod deck;
pub mod edit;
pub mod history;
pub mod instructions;
pub mod protocol;
pub mod session;
pub mod tool;
pub mod workspace;

use std::io::{self, BufRead, BufReader, IsTerminal, Write};
use std::path::PathBuf;

use slidx_jsonrpc::lines::{self, Frame};
use slidx_jsonrpc::Message;

use crate::args::Matches;
use crate::style::Style;
use crate::Outcome;

pub use protocol::{PROTOCOL_VERSION, SUPPORTED};
pub use session::Session;
pub use workspace::Workspace;

/// Name reported to the client at `initialize`.
pub const SERVER_NAME: &str = "slidx";

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    // Typed at a prompt rather than spawned by a client, this would sit there
    // reading a terminal until somebody worked out what had happened. Naming
    // the configuration is the only useful thing to say.
    if io::stdin().is_terminal() {
        return Outcome::misuse(spoken_to_not_typed(style));
    }

    let mut session = Session::new(workspace(matches));
    let mut input = BufReader::new(io::stdin().lock());
    let mut output = io::stdout().lock();

    match serve(&mut input, &mut output, &mut session) {
        // Standard output is the wire, so there is nothing to print. The client
        // closing the stream is how a session ends, and it is not a failure.
        Ok(()) => Outcome::default(),
        Err(error) => Outcome::misuse(format!("slidx mcp: {error}\n")),
    }
}

/// What to say to somebody who typed this at a prompt.
///
/// The configuration rather than an apology: whoever ran it wants the server
/// connected, and the next thing they need is the JSON that connects it.
fn spoken_to_not_typed(style: &Style) -> String {
    format!(
        "{} speaks the Model Context Protocol over standard input and output.\n\
         There is nothing to see here — a client starts it, and this is the\n\
         configuration most of them take:\n\
         \n\
         {{\n\
         \x20 \"mcpServers\": {{\n\
         \x20   \"slidx\": {{ \"command\": \"slidx\", \"args\": [\"mcp\"] }}\n\
         \x20 }}\n\
         }}\n\
         \n\
         It serves the directory it is started in, read-only. Pass --root <path>\n\
         for each other project it may read, and --write to let an agent apply\n\
         slidx edit operations to a deck under a root.\n",
        style.paint(crate::style::Ink::Strong, "slidx mcp")
    )
}

/// The workspace the command line asked for.
///
/// Read-only unless `--write` was passed. That default is the whole point: a
/// client that spawns this server without being asked to allow writes gets a
/// server that cannot make any.
fn workspace(matches: &Matches) -> Workspace {
    let workspace = Workspace::new(roots(matches));

    if matches.is_set("write") {
        return workspace.writing();
    }

    workspace
}

/// The directories the server will open a file under.
///
/// The working directory when nothing is named, because that is the project a
/// client spawned this server for. Every `--root` adds one.
fn roots(matches: &Matches) -> Vec<PathBuf> {
    let named: Vec<PathBuf> = matches.values("root").map(PathBuf::from).collect();

    if named.is_empty() {
        return vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))];
    }

    named
}

/// Reads frames until the client hangs up.
///
/// Takes its streams so a whole session can be a recorded exchange in a test
/// rather than a process somebody has to arrange.
pub fn serve(
    input: &mut impl BufRead,
    output: &mut impl Write,
    session: &mut Session,
) -> io::Result<()> {
    loop {
        let answers = match lines::read(input)? {
            Frame::Message(message) => session.handle(message),
            // Recoverable here in a way it is not with header framing: the next
            // newline is the next frame whatever went wrong before it.
            Frame::Malformed(why) => vec![Message::parse_error(why)],
            Frame::Batch => vec![Session::refuse_batch()],
            Frame::End => return Ok(()),
        };

        for answer in &answers {
            lines::write(output, answer)?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches_for(line: &str) -> Matches {
        let argv: Vec<String> =
            format!("mcp {line}").split_whitespace().map(String::from).collect();

        match crate::args::parse(&argv) {
            crate::args::Invocation::Run(_, matches) => matches,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    #[test]
    fn with_no_root_the_server_serves_the_directory_it_was_started_in() {
        // Which is the project the client spawned it for.
        assert_eq!(roots(&matches_for("")), vec![std::env::current_dir().expect("a cwd")]);
    }

    #[test]
    fn every_root_named_on_the_command_line_is_served() {
        // A speaker reusing a slide from an older talk points at both.
        assert_eq!(
            roots(&matches_for("--root /talks/one --root /talks/two")),
            vec![PathBuf::from("/talks/one"), PathBuf::from("/talks/two")]
        );
    }

    #[test]
    fn a_session_over_an_empty_stream_ends_without_writing_anything() {
        let mut output = Vec::new();
        let mut session = Session::new(Workspace::new(vec![std::env::temp_dir()]));

        serve(&mut "".as_bytes(), &mut output, &mut session).expect("a clean end");

        assert!(output.is_empty(), "a client that said nothing is owed nothing");
    }
}
