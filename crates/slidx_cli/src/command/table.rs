//! Every command slidx has, and the two it declines.
//!
//! Split from the shapes it is built out of because they answer different
//! questions. [`super::Command`] is what a command *is* — the thing the parser,
//! the help text and four completion scripts all read. This is *which*
//! commands there are, which is the list a person edits when they add one.
//!
//! Adding a command is one entry here and one arm in [`crate::run`]. Nothing
//! else needs touching, and there is a test that fails if the two disagree.

use super::{Command, Flag};

const fn leaf(
    name: &'static str,
    summary: &'static str,
    usage: &'static str,
    about: &'static str,
    flags: &'static [Flag],
) -> Command {
    Command { name, summary, usage, about, flags, subcommands: &[], default_subcommand: None }
}

/// Accepted by every command, so they are not repeated in each table.
pub const GLOBAL: &[Flag] = &[Flag::switch("help", "Print this help").short('h')];

/// Accepted before a subcommand only.
pub const ROOT: &[Flag] = &[
    Flag::switch("help", "Print this help").short('h'),
    Flag::switch("version", "Print the version and exit").short('V'),
];

pub const ALL: &[Command] = &[
    leaf(
        "doctor",
        "check this machine before you speak",
        "doctor [options]",
        "\
Reads power, disk, clock, fonts, running applications and the network, and
says what to do about each one. Everything it looks at is something that goes
wrong on stage and never at a desk, so it is worth the ten seconds in the room
even when it was clean this morning.

A reading that could not be taken is reported as unknown, never as a pass.",
        &[
            Flag::taking("dir", "<path>", "Directory whose volume the disk check measures"),
            Flag::switch("offline", "Take no network readings, and say so in the report"),
            Flag::switch("explain", "Add what each check exists to catch"),
            Flag::switch("json", "Print the findings as JSON"),
        ],
    ),
    leaf(
        "lint",
        "check a deck for what a room will do to it",
        "lint [path] [options]",
        "\
Runs every slidx rule over a deck on disk: projector contrast, rendered font
size at the back row, offline assets, heading order, animation cost, and the
time budget against the declared slot.

Exits non-zero when something blocking is found, which is what makes it usable
in CI. `path` is a deck file or a directory of slide files, and defaults to
./slides — the same layout @slidx/vite-plugin builds.",
        &[
            Flag::taking("theme", "<name>", "Theme to resolve colours against"),
            Flag::taking("separator", "<text>", "Slide separator in a single-file deck"),
            Flag::taking("allow", "<code>", "Suppress a rule or a whole group").repeatable(),
            Flag::switch("strict", "Also report advisory findings"),
            Flag::switch("json", "Print the diagnostics as JSON"),
        ],
    ),
    leaf(
        "open",
        "find a deck this machine has seen",
        "open [query] [options]",
        "\
Fuzzy-searches the decks slidx has seen and prints the path of the one you
pick. The index fills itself — running any command on a deck is what puts it
in the list — and a project that has been deleted or moved simply stops
appearing.

Only the chosen path goes to standard output, so this composes:

    cd \"$(slidx open vueconf)\"

Piped, or with --list, it prints every match and exits rather than waiting for
a keypress there is nobody to press.",
        &[
            Flag::switch("list", "Print every match and exit, without the picker"),
            Flag::switch("json", "Print the matches as JSON"),
        ],
    ),
    leaf(
        "tui",
        "walk a deck's structure in the terminal",
        "tui [path] [options]",
        "\
Steps through a deck in the terminal, one stop at a time, drawn inside a box
at the deck's own aspect ratio. Navigation uses the same keys the deck itself
does, so what you learn here works on stage.

This shows STRUCTURE AND FLOW, and nothing about appearance. How many stops a
slide has, what each one reveals, how the deck reads end to end, whether the
bullets are eight when you thought they were four.

It cannot tell you whether text fits, what the contrast is, or how the layout
lands — a terminal row is not a line of 40pt type. Content fitting inside the
box here is not evidence it fits the slide. `slidx lint` checks the room, and
a browser shows the deck.

Piped, it prints one stop and exits rather than waiting for a keypress there
is nobody to press.",
        &[
            Flag::taking("slide", "<number>", "Open on this slide, counting from one"),
            Flag::taking("stop", "<number>", "Open on this stop, counting from one"),
            Flag::taking("separator", "<text>", "Slide separator in a single-file deck"),
        ],
    ),
    leaf(
        "completions",
        "print a completion script for your shell",
        "completions <shell>",
        "\
Writes a completion script to standard output, for bash, zsh, fish or
powershell. It is generated from the same table the parser and the help text
read, so what completes is what actually runs.

Where it goes depends on the shell:

    slidx completions bash > ~/.local/share/bash-completion/completions/slidx
    slidx completions zsh  > ~/.zfunc/_slidx
    slidx completions fish > ~/.config/fish/completions/slidx.fish

    slidx completions powershell >> $PROFILE",
        &[],
    ),
    leaf(
        "preview",
        "look at what the build produced",
        "preview [dir] [options]",
        "\
Opens the exported PDF, or with --web serves the built deck on loopback and
opens a browser at it. `dir` is a build output directory and defaults to
./dist.

Serving rather than opening the files off disk is deliberate: a slide with
more than one stop imports its runtime as a module, and a browser refuses a
module import from a file:// origin. Opened off disk a staged deck sits frozen
on its first stop.

This does not build. When there is nothing there it says so and names
@slidx/vite-plugin, which is the thing that produces a deck.",
        &[
            Flag::switch("web", "Serve the deck and open a browser instead of the PDF"),
            Flag::taking("port", "<number>", "Port to serve on. Default: one the system picks"),
            Flag::switch("no-open", "Print where it is rather than opening anything"),
        ],
    ),
    leaf(
        "publish",
        "do the half of publishing that needs no account",
        "publish [path] [options]",
        "\
Plans all six destinations from frontmatter the author already wrote, and then
performs the four that are files on their own disk: the blog scaffold assembled
from the speaker notes, the resources page built from every link in the deck,
the talk's archive record, and the index over every record beside it.

Speaker Deck and Docswell need an account. slidx composes what to send them,
prints it as fields to paste, and names the page to paste it into — it stores
no token and makes no network call, because a tool that can post as you is a
tool that has to be trusted with a credential.

Exits non-zero when a destination is blocked, naming the frontmatter key that
would unblock it. Waiting on a person is not a failure. `--plan` writes
nothing, so it can be read before it is meant and diffed against last time.",
        &[
            Flag::taking("out", "<path>", "Directory the written pages go under"),
            Flag::taking("pdf", "<path>", "The built PDF the slide hosts take"),
            Flag::taking("card", "<path>", "The social card image to attach"),
            Flag::taking("target", "<name>", "Publish one destination; repeatable").repeatable(),
            Flag::taking("separator", "<text>", "Slide separator in a single-file deck"),
            Flag::switch("plan", "Print what would happen and write nothing"),
            Flag::switch("open", "Open the upload page for each account you have to use"),
            Flag::switch("json", "Print the plan as JSON"),
        ],
    ),
    Command {
        name: "version",
        summary: "install and switch between slidx versions",
        usage: "version [<command>] [options]",
        about: "\
Keeps several slidx versions side by side under ~/.slidx/versions and points
~/.slidx/bin/slidx at the one in use, so a talk can be rehearsed and given
against the version it was built with.

A project pins its version in a .slidx-version file, found by walking up from
wherever you are — so any command run inside a repository picks up the pin at
its root. Failing that, `slidx version use` sets the default for the machine.

With no command it reports what is running and where that binary came from,
which is `slidx version current`.",
        flags: &[],
        default_subcommand: Some("current"),
        subcommands: &[
            leaf(
                "current",
                "say what is running, and who is in charge of it",
                "version current [options]",
                "\
Reports the running binary, the file it actually is, and which install channel
put it there — then says plainly whether `slidx version use` can change it.

That last part is the point. `npm i -g slidx` earlier on your PATH will win
over a managed install and nothing else will ever mention it, so a version
manager that cannot tell you it is not in charge is worse than none.",
                &[Flag::switch("json", "Print the report as JSON")],
            ),
            leaf(
                "list",
                "show the versions installed on this machine",
                "version list [options]",
                "\
Every version under ~/.slidx/versions, newest first, marking the one in use and
the one currently running. A directory with no binary in it is a half-finished
install and is not listed.",
                &[Flag::switch("json", "Print the list as JSON")],
            ),
            leaf(
                "install",
                "download a version and verify it",
                "version install <version> [options]",
                "\
Downloads the release archive for this machine's target and checks it against
the SHA256SUMS published with that release — the same file `install.sh` reads.
A mismatch, or an archive the checksum file does not mention, installs nothing.

Verification is not optional and has no fallback: slidx computes the digest
itself rather than looking for sha256sum on the machine.

Does not change which version is in use. `--use` does that in the same breath.",
                &[
                    Flag::switch("use", "Switch to it once it is installed"),
                    Flag::switch("force", "Download again even if it is already installed"),
                ],
            ),
            leaf(
                "use",
                "switch to an installed version",
                "version use <version>",
                "\
Points ~/.slidx/bin/slidx at an installed version and records the choice in
~/.slidx/version. A project's .slidx-version still wins over this inside that
project — the default is what applies everywhere else.",
                &[],
            ),
            leaf(
                "remove",
                "delete an installed version",
                "version remove <version>",
                "Deletes a version from ~/.slidx/versions. Refuses to remove the one in use.",
                &[],
            ),
        ],
    },
];

/// Commands slidx deliberately does not have, and where the work actually is.
///
/// Somebody typing one of these has a real need, and "unknown command" would
/// leave them hunting for a flag that is never coming. Naming the tool that
/// does own the job answers the question in one line.
///
/// The build pipeline is the Vite plugin. A second implementation of it here
/// would be two answers to one question — the artifact a speaker stands in
/// front of has to come from one place.
pub const DECLINED: &[(&str, &str)] = &[
    ("build", BUILD_LIVES_IN_THE_PLUGIN),
    ("dev", BUILD_LIVES_IN_THE_PLUGIN),
    ("serve", BUILD_LIVES_IN_THE_PLUGIN),
    ("export", BUILD_LIVES_IN_THE_PLUGIN),
    ("pdf", BUILD_LIVES_IN_THE_PLUGIN),
];

const BUILD_LIVES_IN_THE_PLUGIN: &str = "\
Building a deck belongs to @slidx/vite-plugin, and slidx will not grow a second
copy of it:

    npm i -D @slidx/vite-plugin

    // vite.config.ts
    import { slidx } from \"@slidx/vite-plugin\";
    export default { plugins: [slidx()] };

`vite dev` serves the deck and `vite build` emits the static deck, the PDF and
the OG images.";

pub fn find(name: &str) -> Option<&'static Command> {
    ALL.iter().find(|command| command.name == name)
}

/// The reason slidx does not have this command, if it is one of the declined.
pub fn declined(name: &str) -> Option<&'static str> {
    DECLINED.iter().find(|(candidate, _)| *candidate == name).map(|(_, reason)| *reason)
}

/// Every command name, for the help text and for shell completions.
pub fn names() -> Vec<&'static str> {
    ALL.iter().map(|command| command.name).collect()
}
