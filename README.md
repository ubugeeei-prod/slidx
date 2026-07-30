<p align="center">
  <strong>slidx</strong><br>
  <em>Slide + DX — the whole life of a talk, not just the slides</em>
</p>

<p align="center">
  Markdown decks that compile to static HTML pages, a visual editor over the<br>
  same file, and a Rust pipeline underneath. One URL per slide, no client router.
</p>

---

> [!NOTE]
> slidx is an independent personal project by [ubugeeei](https://github.com/ubugeeei), built on
> the [Ox Content](https://github.com/ubugeeei-prod/ox-content) Markdown engine.
> It is pre-alpha and **unreleased** — everything marked shipped below is built
> and tested, and nothing is on npm or crates.io yet.

## Why this exists

**Making the slides is the short part.** Giving the talk is the long one, and
almost everything that goes wrong happens outside the editor:

|                                            |                                            |
| ------------------------------------------ | ------------------------------------------ |
| the venue Wi-Fi is down                    | and the deck's fonts were on a CDN         |
| the body text was 18px                     | and unreadable from row 12                 |
| the projector washed out a colour pair     | that looked fine on a laptop               |
| the build collapsed into one page          | and the animation is gone from the PDF     |
| presenter view refused to open             | because the venue forced display mirroring |
| the demo died                              | and there was no fallback                  |
| the code on screen was unreadable          | and nobody could take it away              |
| publishing afterwards was six manual steps | done exhausted, so it never happened       |

Every one of those is a **build-time or runtime concern of the framework**, not
something a speaker should be handling ten minutes before they walk on. That is
the whole thesis: a presentation tool should know what a conference room does to
a slide, and what a speaker needs between walking up and sitting down.

And a speaker does not give one talk. They keep five decks in five
repositories, revisit one a year later, and reuse a third — so slidx keeps an
index of the decks on your machine rather than assuming the one in front of you
is the only one.

## Start here

```bash
vp add -D @slidx/vite-plugin
```

```ts
// vite.config.ts
import { defineConfig } from "vite";
import { slidx } from "@slidx/vite-plugin";

export default defineConfig({ plugins: [slidx()] });
```

That is the whole configuration. `slidx()` finds `./slides`, serves the deck and
the visual editor in dev, and on build emits static HTML and social cards. A PDF
too, once you ask for one — it needs a browser, so it is opt-in rather than
something every build pays for.

```bash
vp dev     # the deck, plus the editor at /__slidx/
vp build   # one HTML document per slide, and nothing that needs a network
```

The binary is separate and optional — it does the things a build cannot:

```bash
curl -fsSL https://raw.githubusercontent.com/ubugeeei-prod/slidx/main/install.sh | sh
```

```bash
npm i -g slidx
```

## What you get

slidx is not a slide renderer with extras. It is the toolchain for **the whole
life of a talk** — proposing it, writing it, checking it, giving it, and the
week afterwards when the recording appears. Here is all of it, and where each
part actually is.

### The output

**Nothing of slidx is in it.** One HTML document per slide, no client router, no
framework runtime. A slide that wants Vue, React, Svelte, Angular or Three.js
opts in for itself and the others never pay for it.

**Native speed, Markdown engine included.** Parser, step compiler, highlighter,
linter and renderer are Rust, reached through one WebAssembly module — so the
editor's live preview and the production build execute the same code.

| Deck       | Build      | Per slide |
| ---------- | ---------- | --------- |
| 100 slides | **28 ms**  | 0.28 ms   |
| 500 slides | **133 ms** | 0.27 ms   |

`node scripts/bench-build.mjs` reproduces those, so the number is measured
rather than remembered.

**Offline by design.** A built deck makes zero network requests — fonts, images,
styles and scripts inlined or bundled, a remote asset is a lint error, and
`browser.test.ts` records every request in Chromium, Firefox and WebKit and
asserts none of them leaves `file://`.

**Built to be found.** One URL per slide is already the hard part of being
indexable. Canonical links, per-slide descriptions, `rel="prev"`/`rel="next"`,
Open Graph and Twitter cards pointing at the card that slide already has, inline
JSON-LD, a sitemap and a `robots.txt`.

### Writing

**A visual editor over the same file.** Not import and export — two views of one
document. An edit is a **byte-range splice into the file you saved**, so your
blank lines, your `*` bullets and your hand-wrapped paragraphs survive untouched.
There is a test that drives eight edits through a real repository and asserts the
whole session removed _two_ lines. Outline, canvas, inspector, an animation
timeline whose playhead scrubs, and the deck's own git history with the page
rendered as it was at any commit.

**The dialect, fully served.** A language server with diagnostics as you type,
completion derived from the Rust enums rather than a second list, symbols and
hover. A formatter that touches **only what slidx owns** — frontmatter order,
separators, step markers, attribute order — and never your prose, because it
emits the same splice everything else does. And a type check for the dialect
itself: a `steps:` entry naming a mark no slide declares, a theme or layout that
resolves to nothing, a duration whose units do not parse.

**A linter that checks the room, not the monitor.** Contrast through a model of
projector washout. Font size by the angular size a glyph subtends from the back
row. Overflow measured in a real browser, per layout region. Images blown up past
their own pixels. Animations that will not stay on the compositor. Time budgets
summed against the slot you were given.

**Themes as tokens, not stylesheets.** Four built in, each carrying light and
dark because the room's lighting is unknown until the day. Every colour is a
token, the palettes are _mixed_ from four numbers through OKLCh rather than
picked, and the built-ins are held to the linter's own rules — so a theme cannot
ship something illegible. A check fails the build on a shadow, a gradient, or a
colour lifted from a framework's defaults.

### The room

**A pre-flight check where you are standing.** `slidx doctor` reads power, disk,
clock skew, the fonts your theme names, whether anything is recording the screen,
and whether the network works. Worst first, each with what to do about it.

**Time you can act on.** A clock against the declared slot that warns before the
end rather than as it expires, a behind/ahead reading against the per-slide
budgets you wrote, the slides you marked optional when you need to drop one, and
rehearsal recording that diffs where the time went against where you planned it.

**Code you can read, and take away.** A shared fence is highlighted at build
time, published as its own page inside your deck's own output, and a QR on the
slide points at it — carrying the _whole_ snippet rather than the part that
fitted.

**The audience, in real time.** A moderated question and reaction channel, and a
demo fallback you declared: a live target and a recording of it working, so a
dead demo is one keypress rather than an apology.

### Across talks

**A speaker does not give one talk.** They keep five decks in five repositories,
revisit one a year later, and reuse a third. `~/.slidx` keeps an index that fills
itself as you work: `slidx list` tables them with slide count and slot,
`slidx grep` searches every one and answers **in slides rather than line
numbers**, `slidx cd` resolves a query to a directory, and `create`, `add`, `mv`
and `save` manage them without you composing Markdown by hand.

**One command from finished deck to published.** `slidx publish` plans every
destination from the frontmatter you wrote at proposal time, performs everything
that needs no account, and hands you the payload for the ones that do. There is
**no HTTP client and no token store** anywhere under it — a tool that can post as
you is a tool that has to be trusted with a credential.

**The file somebody else asked for.** `slidx export --target <browser|pdf|pdf-zip|png|pptx>`,
each out of the build you already ran rather than a second rendering, so what you
hand over is what you rehearsed.

**Everything from the CLI, including for an agent.** Fourteen commands, complete
help and completions for six shells, a terminal UI, and `slidx mcp` — where every
mutation is an edit operation that hands back its own inverse, so an agent
editing your talk is structurally incapable of reflowing a paragraph.

## Where each part is

|                                                                    |                                                   |
| ------------------------------------------------------------------ | ------------------------------------------------- |
| Framework-independent output                                       | **shipped**                                       |
| Native build, Markdown engine included                             | **shipped**                                       |
| Managed index across decks — list, grep, cd, create, add, mv, save | **shipped**                                       |
| Code on a slide, shared as a page behind a QR                      | **shipped**                                       |
| Real-time audience channel                                         | **shipped**                                       |
| Visual editor — outline, canvas, inspector, timeline, history      | **shipped**; direct manipulation in progress      |
| Time budgets, pacing, rehearsal                                    | **shipped**                                       |
| LSP, formatter, and a type check for the dialect                   | **shipped**                                       |
| Linter for legibility, overflow, timing                            | **shipped**                                       |
| Built-in themes as tokens                                          | **shipped**; npm-distributable themes in progress |
| Everything reachable from the CLI                                  | **shipped**                                       |
| Deploy assistance for slide platforms                              | **shipped** (payloads; uploads stay yours)        |
| SEO artefacts — canonical, sitemap, cards, structured data         | in review                                         |
| Automatic translation                                              | in review                                         |
| System panel — display, mirroring, Do Not Disturb, volume          | in progress                                       |
| Speaker camera on the slide                                        | in progress                                       |

Nothing is released yet. The five properties none of this will trade away — one execution of one model,
steps as snapshots rather than deltas, parsing that never fails, offline by
default, and the canvas and the file being one document — are on the
documentation site under **Choosing slidx**, along with a section naming five
reasons to use something else.

[ROADMAP.md](./ROADMAP.md) is the honest version: every
unchecked line says _why_ it is not done, and it opens with what a checked box is
allowed to mean — a distinction this project learned the hard way, four times.

## What it looks like

Emitted by `vp build` in [examples/deck](./examples/deck), whose entire
configuration is `plugins: [slidx()]`. `node scripts/screenshot.mjs`
regenerates these, so an image that stopped being true fails to reproduce rather
than quietly misleading.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="./docs/images/2-dark.png">
  <img alt="A slide titled 'What actually goes wrong' listing five stage failures" src="./docs/images/2-light.png">
</picture>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="./docs/images/4-dark.png">
  <img alt="A slide with a table of linter rules and what each one catches" src="./docs/images/4-light.png">
</picture>

### The speaker's view

Ordered by how urgently a question needs answering mid-sentence, not by how much
space the answer needs. The clock is the largest thing on the page; the notes get
more room than the slide, because the slide is already on the wall behind you and
what you cannot see is what you meant to say about it.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="./docs/images/2-presenter-dark.png">
  <img alt="The presenter view: a large clock reading 0:00 of 20m, speaker notes, and a preview of the next slide" src="./docs/images/2-presenter-light.png">
</picture>

Arrow keys drive the deck from here, because that is where a clicker sends them.
A second window stays on the same slide over a broadcast channel; where that
channel is unavailable, mirroring is off and the deck still presents.

## The shape of a deck

```md
---
title: Making Decks Fast
event: SlidxConf 2026
duration: 20m
theme: editorial
aspect: "16:9"
---

# Making Decks Fast

<!-- notes:
Open with the outcome, not the agenda.
-->

---

autoSteps: list
budget: 90s
---

## What we will cover

- Why the parser matters
- What the linter catches
- How publishing collapses into one command
```

Each `<!-- step -->`, `autoSteps:` mode, or explicit `steps:` list compiles into
the same timeline, so the animation you author is the animation that prints.

### Addressing part of a line

A _mark_ names a range inside a block — the smallest thing an editor can point
at, and the reason selecting three words and colouring them has somewhere to go
in the file:

```md
The result was [3.2x faster]{#result .accent}.
```

### Changing something already on screen

Revealing covers "not there yet". For a value that _changes_, write the takes
next to each other and they become one element with successive states:

```md
Latency dropped to [120ms]{#latency}[38ms]{#latency}.
```

One DOM node whose text changes, not two that swap. Stepping backwards shows the
earlier value again, because each stop is a complete snapshot and the runtime
keeps no history.

For properties rather than content, say so in the timeline:

```md
---
steps:
  - set: { target: "#latency", color: success }
---
```

### Sharing a fence

```rust {#retry-policy .share title="How we back off"}
fn retry(attempt: u32) -> Duration { … }
```

`.share` publishes the snippet as its own page in the deck's output and draws a
QR on the slide pointing at it.

## The `slidx` command

Three jobs, and none of them is building a deck. **Writing** one, when you would
rather not leave the terminal. **The room** — what is about to happen to your
talk that no editor can see. And **the decks you already have**, because a
speaker who gives four talks a year has four repositories and remembers where
none of them are.

```bash
slidx dev         # write the deck, with the editor open
slidx fmt         # normalise the parts slidx owns, and nothing else
slidx tui         # step through a deck's structure in the terminal
```

```bash
slidx doctor      # check the machine you are about to speak from
slidx lint        # every rule the build runs, exiting non-zero on anything blocking
slidx preview     # open the built PDF, or --web to serve the deck on loopback
slidx export      # package what the build produced, for somewhere else
slidx publish     # plan, and perform the half that needs no account
```

```bash
slidx list        # every deck this machine has seen: slides, slot, event, when
slidx grep        # search them all, and get back the slide rather than the line
slidx cd          # print a deck's directory, for a shell function to enter
slidx open        # fuzzy-find a deck and print its path
slidx version     # install and switch between slidx versions
```

`slidx grep` is the one worth explaining. `slides/0007.md:12` is not where a
speaker keeps their content — "slide 7 of the VueConf deck" is — so the search
parses what it matches and answers in slides. It reads decks and not
repositories: `node_modules`, build output and dot directories are skipped, and
a deck is parsed only once a line in it has already matched, which is what makes
it fast enough to type on a whim.

`slidx cd` prints a path rather than changing directory, and that is not a
limitation waiting to be fixed: **a child process cannot change the working
directory of the shell that started it.** So it resolves, and a shell function
enters — the same pair every directory jumper is built out of.

Either install channel hands over the same prebuilt binary — no Node, no
compiler, and **no `postinstall` that downloads anything**. The shell installer
verifies the download against the SHA-256 published with the release and never
asks for sudo; `--dry-run` prints where it would put the binary and stops.

It installs into `$SLIDX_HOME`, else `$XDG_DATA_HOME/slidx`, else `~/.slidx` —
and `%LOCALAPPDATA%\slidx` on Windows. `slidx version` resolves the same path in
the same order, which is what lets the version manager ever be in charge of the
binary you are running.

### In your shell

```bash
slidx completions <shell>   # what `slidx <tab>` offers
slidx shell <shell>         # the function that lets slidx move you
```

Both are generated from the one table the parser reads, so what completes is
what runs. Run either with no argument and it lists the shells it knows and
which file the output belongs in — the list is not repeated in this README,
because a list in prose is a list that goes stale.

Two of those shells are honest special cases. **sh** has no programmable
completion and no script will ever give it one, so it is told so in a sentence
rather than handed a stub that installs cleanly and changes nothing. **ush**
completes from a catalogue compiled into itself, so what slidx writes is that
catalogue entry, in ush's own shape, for its maintainer to adopt.

The integration is the other half, and every shell has it: a process cannot
change its parent's working directory, so a command that finds a deck can only
print where it is. One shell function closes that gap, and it is derived from
the table too — a command that needs your shell gets it in every shell at once.

Prebuilt for macOS on Apple silicon and Intel, Linux on x86-64 and ARM64
(statically linked, so Alpine works), and Windows on x86-64. Anywhere else,
`cargo install slidx_cli` builds it.

`dev` and `preview` are easy to confuse and answer different questions. `dev`
serves your slide files live with the editor at `/__slidx/` and can write to
them; `preview` serves what the build already produced and never writes
anything. One shows you the deck you are writing, the other shows you what the
projector will show.

**There is deliberately no `slidx build`.** That is `@slidx/vite-plugin`'s job,
and one pipeline is the whole point. `slidx dev` is not the exception it looks
like: it implements no server. It finds your Vite config by walking up from the
deck, works out which package manager filled `node_modules` by reading the
lockfile, hands the terminal to `vite dev` with the plugin, and gets out of the
way. Every byte of HTML still comes from one place.

`slidx export` is not a second pipeline wearing another name either: it _runs_
that build and packages what it wrote, so without the plugin installed it
produces nothing and says so. Every page, every PDF and every image in an export
came out of the same browser pass over the same print shell as the deck you
rehearsed.

## Development

[Vite+](https://voidzero.dev) runs every task in this repository, Rust and
TypeScript alike. One command runs exactly what CI runs:

```bash
vp run workspace:ci
```

See [CONTRIBUTING.md](./CONTRIBUTING.md) for the method and the layout rules,
and [RELEASING.md](./RELEASING.md) for how a release is cut.

## License

MIT
