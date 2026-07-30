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
    Command {
        name,
        summary,
        usage,
        about,
        flags,
        subcommands: &[],
        default_subcommand: None,
        takes_the_caller_with_it: false,
    }
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
        "help",
        "describe a command, or list them all",
        "help [command]",
        "\
Prints the page for one command, or the list of them when given nothing.

    slidx help lint
    slidx help version install

`slidx lint --help` prints the same page. Both read this table, so they cannot
disagree — and there is a test that fails if they ever do.

Which spelling somebody reaches for depends on what they used last, so both
work rather than one being the real one.",
        &[],
    ),
    leaf(
        "doctor",
        "check this machine before you speak",
        "doctor [options]",
        "\
Reads power, disk, clock, fonts, running applications and the network, and
says what to do about each one. Everything it looks at is something that goes
wrong on stage and never at a desk, so it is worth the ten seconds in the room
even when it was clean this morning.

A reading that could not be taken is reported as unknown, never as a pass.

    slidx doctor --explain
    slidx doctor --dir ~/talks/vueconf --offline",
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
./slides — the same layout @slidx/vite-plugin builds.

    slidx lint
    slidx lint ./slides --theme editorial --strict
    slidx lint --allow contrast --allow structure/missing-alt",
        &[
            Flag::taking("theme", "<name>", "Theme to resolve colours against"),
            Flag::taking("separator", "<text>", "Slide separator in a single-file deck"),
            Flag::taking("allow", "<code>", "Suppress a rule or a whole group").repeatable(),
            Flag::switch("strict", "Also report advisory findings"),
            Flag::switch("json", "Print the diagnostics as JSON"),
        ],
    ),
    leaf(
        "fmt",
        "normalise the parts of a deck slidx owns",
        "fmt [path] [options]",
        "\
Rewrites frontmatter key order and indentation, the slide separator's
spelling, step marker spelling, the attribute order inside a mark's braces,
and the shape of a notes comment. `path` is a deck file or a directory of
slide files, and defaults to ./slides.

It is NOT a Markdown formatter and will not become one. Your prose, your line
wrapping, your bullet markers, your table alignment and everything inside a
fenced code block come out byte for byte, because slidx does not own them and
a diff nobody asked for is how a tool loses the right to touch a file.

--check writes nothing and exits non-zero when a file is not already
formatted, which is the form for CI. Each file is formatted on its own, so a
deck kept as one file per slide stays that way.",
        &[
            Flag::switch("check", "Write nothing; exit non-zero if a file would change"),
            Flag::taking("separator", "<text>", "Slide separator in a single-file deck"),
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

With `slidx shell` loaded you are simply taken there, because the shell
function can do the one thing this command cannot.

Piped, or with --list, it prints every match and exits rather than waiting for
a keypress there is nobody to press.",
        &[
            Flag::switch("list", "Print every match and exit, without the picker"),
            Flag::switch("json", "Print the matches as JSON"),
        ],
    )
    .taking_the_caller_with_it(),
    leaf(
        "list",
        "show the decks on this machine",
        "list [options]",
        "\
Every project in the index, most recently touched first, with the things that
tell one deck from another: how many slides it has, how long the slot is, when
it was last worked on, and the event it was written for.

The index fills itself — running any command on a deck is what puts it in the
list — so nothing has to be registered, and a project that has been deleted or
moved stops appearing on its own.

Slide counts and durations are read out of the decks rather than remembered, so
a number in the table is the number in the file.",
        &[Flag::switch("json", "Print the list as JSON")],
    ),
    leaf(
        "cd",
        "print a deck's directory, for a shell to enter",
        "cd [query]",
        "\
Fuzzy-finds a project and prints its directory, which is all a program can do
here: a child process cannot change the working directory of the shell that
started it, and no flag will make it. That is how processes work rather than
something missing, so the `cd` belongs to a shell function that reads this
command's output — and directly, to a command substitution:

    cd \"$(slidx cd vueconf)\"

The quotes are not optional. A deck kept in a directory whose name has a space
in it is otherwise split into two arguments, and `cd` is handed the first half.

Exactly one path is printed, ever. A query matching several projects opens a
picker — on the terminal, which is why it works inside a substitution — and
where there is no terminal to pick on it takes the closest match and names it on
standard error. A query matching nothing prints nothing and exits non-zero, so a
substitution fails loudly rather than entering the empty string.",
        &[],
    ),
    leaf(
        "grep",
        "search every deck this machine has seen",
        "grep <text> [options]",
        "\
Searches the deck sources of every project in the index and reports the SLIDE a
match is on, not just the line: a line number in a Markdown file is not where a
speaker keeps their content, and `slide 7 of the VueConf deck` is.

Plain text, matched anywhere in a line — there is no pattern syntax to learn
and none to escape. A query in all lowercase matches either case; a query with
a capital in it is matched exactly, so `Vue` finds the framework and not
`revue`.

Only decks are read: `node_modules`, build output and dot directories are
skipped, which is what keeps this fast enough to type on a whim rather than
schedule.",
        &[
            Flag::taking("limit", "<number>", "Stop after this many matches. Default: 100"),
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
is nobody to press.

    slidx tui
    slidx tui ./slides --slide 4 --stop 2",
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
Writes a completion script to standard output. It is generated from the same
table the parser and the help text read, so what completes is what actually
runs.

    slidx completions fish > ~/.config/fish/completions/slidx.fish

With no shell it lists every shell it knows and where that shell's script has
to go. That list is not repeated here, so it cannot go stale here.

A shell with no programmable completion is told so in a sentence rather than
handed a script that would do nothing — and the shell integration, which is a
different thing and works everywhere, is `slidx shell`.",
        &[],
    ),
    leaf(
        "shell",
        "let slidx move the directory you are standing in",
        "shell <name>",
        "\
Writes a shell function to standard output. Load it from your profile:

    eval \"$(slidx shell sh)\"

A process cannot change its parent's working directory. Nothing can, on any
operating system, and no version of slidx will be able to — so a command that
finds a deck can only print where that deck is, and the shell you typed into
is the only thing that can take you there. This function closes that gap: it
runs slidx, prints what slidx printed, and follows it when what came back was
a directory.

Every other command is passed straight through, untouched and unbuffered,
because a report that arrived all at once at the end would be a worse report.

With no name it lists every shell it knows and which file the line goes in.",
        &[],
    ),
    leaf(
        "dev",
        "write the deck, with the editor open",
        "dev [path] [options]",
        "\
Starts the project's own dev server — Vite with @slidx/vite-plugin — and opens
the visual editor at /__slidx/. `path` is the deck and defaults to ./slides,
the same as `slidx lint`; the Vite config is found by walking up from it.

slidx is not a server. It finds the project, reaches Vite through the package
manager that installed it, and names the editor's route, which is the page Vite
has no reason to know about. Everything that renders a slide belongs to the
plugin, because the artifact a speaker stands in front of has to come from one
place.

Use this while WRITING. `slidx preview` is for looking at what a build
produced: the editor writes to your slide files and exists only here, and only
the build output is what a host will actually serve.",
        &[
            Flag::taking("port", "<number>", "Port to serve on. Default: Vite's own"),
            Flag::switch("no-open", "Do not open a browser at the editor"),
        ],
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
@slidx/vite-plugin, which is the thing that produces a deck.

Use this to check the RESULT — the same files a static host would serve. While
you are still writing, `slidx dev` serves the source live and opens the editor.

    slidx preview
    slidx preview ./dist --web --port 4321",
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
nothing, so it can be read before it is meant and diffed against last time.

    slidx publish --plan
    slidx publish --pdf ./dist/deck.pdf --out ./published
    slidx publish --target resources --target archive",
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
which is `slidx version current`.

    slidx version
    slidx version install 0.4.0 --use",
        flags: &[],
        takes_the_caller_with_it: false,
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
manager that cannot tell you it is not in charge is worse than none.

    slidx version current
    slidx version current --json",
                &[Flag::switch("json", "Print the report as JSON")],
            ),
            leaf(
                "list",
                "show the versions installed on this machine",
                "version list [options]",
                "\
Every version under ~/.slidx/versions, newest first, marking the one in use and
the one currently running. A directory with no binary in it is a half-finished
install and is not listed.

    slidx version list",
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

Does not change which version is in use. `--use` does that in the same breath.

    slidx version install 0.4.0
    slidx version install 0.4.0 --use --force",
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
project — the default is what applies everywhere else.

    slidx version use 0.3.0",
                &[],
            ),
            leaf(
                "remove",
                "delete an installed version",
                "version remove <version>",
                "\
Deletes a version from ~/.slidx/versions. Refuses to remove the one in use,
because the next thing you typed would be the shim pointing at nothing.

    slidx version remove 0.2.0",
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
///
/// `dev` used to be on this list and is now a command, and the distinction is
/// worth being precise about: it does not *implement* a dev server, it starts
/// the project's own. Every name still here would have had to build something.
pub const DECLINED: &[(&str, &str)] = &[
    ("build", BUILD_LIVES_IN_THE_PLUGIN),
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

`vite build` emits the static deck, the PDF and the OG images. To serve the deck
while you write it, `slidx dev` starts that same dev server for you.";

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

/// Every command whose output the shell that called it has to act on.
///
/// The list [`crate::shell::integration`] writes into its wrapper function, in
/// every shell. Derived rather than written down, so a command that gains the
/// property gains it in bash, zsh, fish, PowerShell, Nushell and ush at once —
/// and one that loses it stops being captured everywhere at once too.
pub fn taking_the_caller_with_them() -> Vec<&'static str> {
    ALL.iter().filter(|command| command.takes_the_caller_with_it).map(|c| c.name).collect()
}
