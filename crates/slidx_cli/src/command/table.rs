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
Reads power, the display arrangement, notifications, the audio output, disk,
clock, fonts, running applications and the network, and says what to do about
each one. Everything it looks at is something that goes wrong on stage and
never at a desk, so it is worth the ten seconds in the room even when it was
clean this morning.

A reading that could not be taken is reported as unknown, never as a pass.
Platforms differ most on the three that are settings rather than measurements:
macOS names display mirroring outright, Windows will not say whether its
screens are duplicated, and Windows has no output level a command line can
read. Each of those is reported as unknown with the reason, and never guessed.

It changes nothing. Mirroring, Do Not Disturb and the volume are all things a
speaker may want set, and none of them are set by a command run to find out
what they are — the remedy names the switch instead.

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

It also checks the deck's dialect, which is a different question — whether the
deck says something slidx can carry out. A `duration:` nothing can read, a
theme or transition name that resolves to nothing, a `steps:` entry addressing
a mark that is not there. Those are silent today and found on stage, so they
are reported under `dialect/` and can be switched off on their own with
--allow dialect.

Exits non-zero when something blocking is found, which is what makes it usable
in CI. `path` is a deck file or a directory of slide files, and defaults to
./slides — the same layout @slidxjs/vite-plugin builds.

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
deck kept as one file per slide stays that way.

    slidx fmt
    slidx fmt ./slides --check",
        &[
            Flag::switch("check", "Write nothing; exit non-zero if a file would change"),
            Flag::taking("separator", "<text>", "Slide separator in a single-file deck"),
        ],
    ),
    leaf(
        "lsp",
        "run the language server, for an editor to talk to",
        "lsp",
        "\
Speaks the language server protocol on stdin and stdout: diagnostics as you
type, completion for frontmatter keys and step presets, the deck outline, hover,
and formatting on save. Everything it reports is what `slidx lint` and
`slidx fmt` report, from the same rules, at the moment it is still cheap to act
on.

An EDITOR runs this, not a person. It takes no arguments and reads no
configuration — an editor starts it and everything else is protocol — and typed
at a prompt it says so rather than waiting for a frame that is not coming. The
configuration for VS Code, Zed and Neovim is in docs/content/editors.md.

It serves Markdown under a slides directory and nothing else. A deck is
Markdown and most Markdown is not a deck, so a language server that claimed
every .md file would put slide diagnostics on somebody's README.

An editor's configuration names it, so the worked example is the line that goes
in one:

    slidx lsp",
        &[],
    ),
    leaf(
        "mcp",
        "serve slidx to an agent over the Model Context Protocol",
        "mcp [options]",
        "\
Speaks the Model Context Protocol over standard input and output, so an agent
can read and check a deck through slidx rather than around it. A client starts
it; typing it at a prompt prints the configuration instead of waiting.

The reason it exists is the same reason the visual editor does. An agent that
edits a deck by rewriting the file regularises the author's blank lines, their
bullets and their hand-wrapped paragraphs — invisible on a slide and enormous in
the diff. So this serves slidx's own operations rather than a file writer, and
an agent working through it cannot reflow a paragraph it did not mean to touch.

It opens no port and makes no outbound request. It reads decks under the
directory it was started in and under the projects this machine has already run
a slidx command on, and nothing else.

Read-only unless --write is passed, and then only under a directory it was
started in or pointed at. Every change it makes is a slidx edit operation that
hands back the edit reversing it, so `undo` takes the last one back byte for
byte — but a deck under version control is still the real safety net.

A client's configuration names it, so the worked example is the line that goes
in one:

    slidx mcp
    slidx mcp --write --root ~/talks",
        &[
            Flag::taking("root", "<path>", "A directory it may read decks under; repeatable")
                .repeatable(),
            Flag::switch("write", "Let an agent apply slidx edit operations to a deck"),
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

    slidx open vueconf | xargs -o $EDITOR

With `slidx shell` loaded you are simply taken there, because the shell
function can do the one thing this command cannot.

Piped, or with --list, it prints every match and exits rather than waiting for
a keypress there is nobody to press. That is what makes it a list to feed to
something — and why `slidx cd` is the one to put inside `cd \"$(…)\"`: it prints
exactly one path however many decks match, and a substitution has no way to
hold two.",
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
a number in the table is the number in the file.

    slidx list
    slidx list --json",
        &[Flag::switch("json", "Print the list as JSON")],
    ),
    leaf(
        "create",
        "start a deck that already builds",
        "create <path> [options]",
        "\
Makes a project at `path`: a deck, a vite config that is one line long, and a
package.json that names the plugin. It is registered in the index on the way
out, so it is in `slidx list` before anything else has been run on it.

What it leaves behind parses, lints clean and builds. A scaffold that needs
fixing before it works is a scaffold that has to be read, and the first thing
somebody does with a new deck should be writing the talk.

Everything you pass goes into the frontmatter through the same edit operations
the visual editor writes through, so a title with a colon in it is quoted the
way YAML needs rather than the way a template guessed.

It installs nothing and runs no package manager. That is the author's to choose,
and it is the one step that needs the network.

    slidx create ./vueconf-2026
    slidx create ./talk --title \"Reactivity from scratch\" --duration 40m",
        &[
            Flag::taking("title", "<text>", "The deck's title. Default: the directory name"),
            Flag::taking("event", "<name>", "The event this talk is for"),
            Flag::taking("duration", "<length>", "The slot, written as 20m or 45m"),
            Flag::taking("theme", "<name>", "minimal, editorial, terminal or contrast"),
        ],
    ),
    leaf(
        "theme",
        "list the built-in themes, or check a theme document",
        "theme [path]",
        "\
With no argument, every theme slidx ships and what each one is for. A theme
package adds a name to that list for the project that installed it, and this
command cannot see those: resolving a package name means resolving node_modules.
@slidxjs/vite-plugin owns that work so it stays in one place.

With a path to a theme document — the JSON file a package names under
`slidx.theme` in its own package.json — it reads that file exactly as a build
would, and reports what slidx would do with it: anything the guard would not
pass through, and everything the linter says about its colours and its type.

That last part is what a theme's author wants and a build cannot give them. A
build judges a theme in the room the deck is being built for; a published theme
is shown in all of them, so this runs every room slidx models. It exits
non-zero when it found something, which is the form for CI.

    slidx theme               # the theme document beside you
    slidx theme ./my-theme    # one somewhere else",
        &[],
    ),
    leaf(
        "add",
        "add a slide to a deck",
        "add [path] [options]",
        "\
Adds a slide, and composes none of it. The bytes are a splice computed by the
same operation set the visual editor writes through, which is what keeps an
author's blank lines, their `*` bullets and their hand-wrapped paragraphs
untouched — a second writer of deck Markdown is the one thing that would break
that, quietly, in somebody's diff months later.

A deck kept as one file per slide gets one new file, and the files after the new
slide move along a number so the deck stays in order. `path` is a deck file or a
directory of slide files, and defaults to ./slides.

`--at` counts from one, the way a speaker counts slides.

    slidx add --title \"The demo\"
    slidx add ./slides --title Recap --at 3",
        &[
            Flag::taking("title", "<text>", "The slide's heading"),
            Flag::taking("at", "<number>", "Where it goes, counting from one. Default: the end"),
            Flag::taking("notes", "<text>", "What the speaker says over it"),
            Flag::taking("separator", "<text>", "Slide separator in a single-file deck"),
        ],
    ),
    leaf(
        "mv",
        "rename a project, and the deck with it",
        "mv <query> <name> [options]",
        "\
Renames a project's directory and follows it in the index, so the deck you look
for tomorrow is the one you renamed today. `name` is a new directory name beside
the old one, or a path when the project is moving somewhere else.

With --title the deck's own title changes too, through the same edit operation
the editor uses. A rename that leaves the title slide saying the old name is
half a rename, and the half left over is the one an audience sees.

Nothing is overwritten: a destination that already exists is a refusal, not a
merge.

    slidx mv vueconf vueconf-2026
    slidx mv vueconf ../talks/vueconf --title \"Reactivity from scratch\"",
        &[Flag::taking("title", "<text>", "Retitle the deck's frontmatter as well")],
    ),
    leaf(
        "rm",
        "archive a project, reversibly",
        "rm [query] [options]",
        "\
Moves a project into an archive under ~/.slidx and records where it came from,
so `slidx rm --restore` puts it back exactly where it was. Nothing is unlinked.

That is deliberate, and it is not timidity. A deck is often the only copy of
work that took weeks: written at night, not always in a repository, and in a
repository that has usually never been pushed. An archive somebody meant to
delete costs disk space; a delete somebody meant to archive costs the talk.

    slidx rm vueconf              archive it
    slidx rm --restore vueconf    put it back
    slidx rm --list               what is archived, and where it came from

--delete really deletes, and asks for the project's name to be typed back
rather than accepting a keypress. A project holding changes that are in no
commit is asked about twice, because that is the case where the copy being
deleted is the only one. Where there is no terminal to ask on, it deletes
nothing.",
        &[
            Flag::switch("restore", "Put an archived project back where it was"),
            Flag::switch("list", "Show what is archived"),
            Flag::switch("delete", "Really delete, after confirming"),
        ],
    ),
    leaf(
        "save",
        "commit the deck, described in the deck's own terms",
        "save [path] [options]",
        "\
Commits the deck and writes the message itself — the part git cannot do. git
sees lines; slidx has a parser, so the commit says `Add two slides and retime
the demo` rather than `+34 -6`. Slides added, dropped or reordered, budgets
changed, notes written: all of it comes from comparing the deck on disk with the
deck at HEAD.

The message is yours to overrule with --message, and nothing is ever appended to
it — no trailer, no footer, no attribution. It is your record of your own talk.

Only the deck is committed. Something else you had staged stays staged, because
one command sweeping up half-finished work is how a tool loses the right to be
typed without thinking. --all widens it to the whole project.

With no repository it offers to start one rather than failing, which is the
state a deck written this morning is in.

    slidx save
    slidx save --dry-run
    slidx save --all -m \"Retime the demo\"",
        &[
            Flag::taking("message", "<text>", "Use this message instead of the written one")
                .short('m'),
            Flag::switch("all", "Commit everything in the project, not only the deck"),
            Flag::switch("init", "Start a repository when there is none, without asking"),
            Flag::switch("dry-run", "Print the message and commit nothing"),
            Flag::taking("separator", "<text>", "Slide separator in a single-file deck"),
        ],
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
command's output. `slidx shell` writes that function:

    eval \"$(slidx shell sh)\"

Without it, a command substitution does the same job by hand:

    cd \"$(slidx cd vueconf)\"

The quotes are not optional. A deck kept in a directory whose name has a space
in it is otherwise split into two arguments, and `cd` is handed the first half.

Exactly one path is printed, ever. A query matching several projects opens a
picker — on the terminal, which is why it works inside a substitution — and
where there is no terminal to pick on it takes the closest match and names it on
standard error.

A query matching nothing prints nothing and exits non-zero. Read that status:
quoted, an empty answer leaves `cd` where it was; unquoted, the empty word
vanishes and `cd` takes you home.

With `slidx shell` loaded the substitution is unnecessary, and `slidx cd
vueconf` simply takes you there.",
        &[],
    )
    .taking_the_caller_with_it(),
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
schedule.

    slidx grep \"the projector\"
    slidx grep Vue --limit 20",
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
Starts the project dev server with @slidxjs/vite-plugin and opens
the visual editor at /__slidx/. `path` is the deck and defaults to ./slides,
the same as `slidx lint`; the Vite config is found by walking up from it.

slidx is not a server. It finds the project, reaches Vite through the package
manager that installed it, and names the editor's route, which is the page Vite
has no reason to know about. Everything that renders a slide belongs to the
plugin, because the artifact a speaker stands in front of has to come from one
place.

Use this while WRITING. `slidx preview` is for looking at what a build
produced: the editor writes to your slide files and exists only here, and only
the build output is what a host will actually serve.

--crdt shares the deck with the laptop next to you. It binds beyond localhost,
prints a link and a QR code, and puts the editor's changes through one shared
document so an edit from the canvas and a file you saved in your own editor
merge rather than overwrite. The link is READ ONLY: --allow-edit mints a second
link, and only that one can change the deck.

Sharing is on your local network and involves no third party. There is no
tunnel and no flag that adds one — a public URL to an unannounced talk, served
by something that can write your files, is not a switch this should have.

    slidx dev
    slidx dev ./slides --port 5173 --no-open",
        &[
            Flag::taking("port", "<number>", "Port to serve on. Default: Vite's own"),
            Flag::switch("crdt", "Share on this network, read-only, to edit together"),
            Flag::switch("allow-edit", "Also mint a link that may change the deck"),
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
@slidxjs/vite-plugin, which is the thing that produces a deck.

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
        "export",
        "package what the build produced, for somewhere else",
        "export --target <name> [path] [options]",
        "\
Produces one file from a deck: the static site as a zip, the deck as one PDF,
one PDF per slide, one image per stop, or an OOXML presentation with speaker
notes.

It does not render anything. Every page, every PDF and every image comes from
@slidxjs/vite-plugin driving a browser over its print shell. This runs
that build and packages what it wrote. A second renderer here would mean
the file you hand over could differ from the deck you checked, which is the one
failure this whole pipeline is shaped to prevent.

Nothing is uploaded and no account is involved. slidx produces a file and you
open it, the same boundary `slidx publish` holds.

`path` is a deck file or a directory of slide files and defaults to ./slides.
The file lands in the current directory, named for the deck, unless --out says
otherwise.

    slidx export --target pdf
    slidx export --target png ./slides --out ./handout",
        &[
            Flag::taking("target", "<name>", "Required. browser, pdf, pdf-zip, png, pptx"),
            Flag::taking("out", "<path>", "Directory the exported file goes in. Default: ."),
            Flag::taking("dist", "<path>", "Build output directory. Default: dist beside the deck"),
            Flag::taking("separator", "<text>", "Slide separator in a single-file deck"),
            Flag::switch("no-build", "Package what is already in the build output"),
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
        name: "i18n",
        summary: "give the same talk in another language",
        usage: "i18n <command> [options]",
        about: "\
Pulls a deck's prose out into a catalogue, and puts a translated one back —
without touching anything slidx addresses.

That second half is the work. A mark key is an address a `steps:` entry points
at, a fence carries code and a snippet's file name, a link's destination is not
its words, and a slide's id is a slug of its heading — so a translated heading
moves the slide and breaks every deep link and every QR code into the deck.
All of it is replaced by numbered placeholders before a translator ever sees
the text, and `apply` pins any id the translation would have moved.

slidx does not translate. Producing the translation is yours to do, with
whichever tool or person you choose; the catalogue is an ordinary Gettext PO
file, so every translation tool already opens it. Nothing here makes a network
call, and no build ever runs any of this.

    slidx i18n extract --lang ja        # the deck's prose, as a PO file
    slidx i18n apply ja.po --lang ja    # a translation, spliced back",
        flags: &[],
        default_subcommand: None,
        takes_the_caller_with_it: false,
        subcommands: &[
            leaf(
                "extract",
                "write the catalogue a translator works in",
                "i18n extract [path] --lang <tag> [options]",
                "\
Writes one entry per translatable string: the deck's title and description,
every heading, paragraph, bullet, quote and table row, and every speaker note —
because a translated slide with untranslated notes is worse than neither.

Everything else is left out on purpose. Fenced code and inline code, URLs and
image paths, mark keys and classes, step markers, HTML tags, and every
frontmatter key that is vocabulary rather than prose. Where one of those sits
inside a sentence it becomes %1, %2 … — so it cannot be retyped wrongly, and can
still be moved when the grammar needs it.

Run over an existing catalogue, it keeps every translation whose string has not
changed, so re-extracting after fixing a typo does not throw away a week of
somebody's work.

    slidx i18n extract --lang ja
    slidx i18n extract ./slides --lang ko --out ko.po",
                &[
                    Flag::taking("lang", "<tag>", "BCP 47 tag being translated into. Required"),
                    Flag::taking("out", "<path>", "Where to write it. Default: standard output"),
                    Flag::taking("separator", "<text>", "Slide separator in a single-file deck"),
                ],
            ),
            leaf(
                "apply",
                "write the translated deck beside the original",
                "i18n apply [path] --catalogue <file> --out <dir>",
                "\
Splices every translation into the deck as a byte-range change, so the author's
blank lines, their bullet markers and their hand-wrapped paragraphs come through
untouched and the diff is one a reviewer can read. A string nobody has
translated yet is left in the original language rather than blanked, so a
half-finished catalogue is safe to apply.

Then it pins the ids. A slide's id is a slug of its heading, so translating
headings moves slides — including ones nobody translated, when two slides shared
a title. Every id the translation would have moved is written back as `id:` in
that slide's frontmatter, so the translated deck answers at the URLs the
original one published.

A translation that dropped a placeholder is refused rather than written, and
named. Dropping %1 silently drops the mark key it stood for, and a deck whose
`steps:` entry addresses nothing still renders — it just does not animate.

`--out` writes a sibling deck, which is the layout that keeps a translation
change legible: `slides.ja/0001.md` diffs against `slides/0001.md` line for
line. Two things do not come across and are reported instead: per-slide
budgets, because speaking rate is not language independent, and the linter's
overflow verdict, because a slide that fitted in one language may not in
another.

    slidx i18n apply --catalogue ja.po --out slides.ja",
                &[
                    Flag::taking("catalogue", "<path>", "The translated PO file. Required"),
                    Flag::taking(
                        "out",
                        "<path>",
                        "Directory the translated deck goes in. Required",
                    ),
                    Flag::taking("separator", "<text>", "Slide separator in a single-file deck"),
                    Flag::switch("plan", "Say what would change and write nothing"),
                ],
            ),
        ],
    },
    leaf(
        "self-update",
        "install and use the latest stable release",
        "self-update",
        "\
Downloads the newest stable release for this binary's target, verifies it
against the release's SHA256SUMS, starts it once to confirm the version it
reports, then installs and selects it through the version manager.

Nothing is replaced before verification succeeds. A binary installed by npm,
cargo or a system package manager remains that manager's responsibility:
self-update refuses and prints the command for that channel instead of putting
a second slidx later on PATH and pretending the update took effect.

    slidx self-update",
        &[],
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
/// `dev` and `export` used to be on this list and are now commands, and the
/// distinction is worth being precise about: `dev` does not *implement* a dev
/// server, it starts the project's own, and `export` is still not a build — it
/// runs the plugin's and packages what that wrote. Every name still here would
/// have had to build something.
pub const DECLINED: &[(&str, &str)] = &[
    ("build", BUILD_LIVES_IN_THE_PLUGIN),
    ("serve", BUILD_LIVES_IN_THE_PLUGIN),
    ("pdf", PDF_IS_AN_EXPORT),
];

/// `slidx pdf` is a reasonable thing to type and names a real artefact, so it
/// gets its own answer rather than the pipeline lecture. The lecture still
/// applies underneath — `export` runs the plugin's build and packages it — and
/// saying so is what keeps somebody from expecting a renderer in the binary.
const PDF_IS_AN_EXPORT: &str = "\
The PDF is an export of a build:

    slidx export --target pdf

That runs @slidxjs/vite-plugin to render the PDF from its print shell,
and puts the document where you asked for it. `slidx export --help`
lists the other things it can package.";

const BUILD_LIVES_IN_THE_PLUGIN: &str = "\
Deck builds belong to @slidxjs/vite-plugin, and slidx will not grow a
second copy:

    npm i -D @slidxjs/vite-plugin

    // vite.config.ts
    import { slidx } from \"@slidxjs/vite-plugin\";
    export default { plugins: [slidx()] };

`vite build` emits the static deck, the PDF and the OG images. To serve the deck
while you write it, `slidx dev` starts that same dev server for you. To turn that
build's output into one file for somewhere else:

    slidx export --target pdf";

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
