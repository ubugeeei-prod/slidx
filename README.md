<p align="center">
  <strong>slidx</strong><br>
  <em>Slide + DX — Markdown decks that compile to static HTML pages</em>
</p>

<p align="center">
  A visual editor over plain Markdown, a Rust pipeline, and output that is<br>
  ordinary web pages — one URL per slide, no client router, no framework runtime.
</p>

---

> [!NOTE]
> slidx is an independent personal project by [ubugeeei](https://github.com/ubugeeei), built on
> the [Ox Content](https://github.com/ubugeeei-prod/ox-content) Markdown engine.
> It is pre-alpha: the surface below is the target, not a changelog.

## What a deck is

**Pages, not an application.** `vp build` writes one HTML document per slide.
Navigation is the browser following a link, so a slide can be shared,
bookmarked, indexed, opened on a phone, and printed — and it renders before any
script has run, because there is no script to run.

**No framework in the output.** Nothing is required to view a deck, and nothing
is required to write one. A slide that wants Vue, React, Svelte, Angular, or
Three.js opts in for itself; the other slides stay HTML and never pay for it.

**A visual editor over the same file.** The canvas and the Markdown are two
views of one document, not an import and an export. Colour three words, move a
block, add an animation — the diff is a line a reviewer can read, and editing
that line by hand moves the canvas. That is checked as a round-trip property,
not asserted.

**Fast enough not to think about.** The parser, step compiler, linter, and
renderer are Rust, reached through one WebAssembly module:

| Deck       | Build      | Per slide |
| ---------- | ---------- | --------- |
| 100 slides | **28 ms**  | 0.28 ms   |
| 500 slides | **133 ms** | 0.27 ms   |

`node scripts/bench-build.mjs` reproduces those, so the number in this table is
measured rather than remembered.

**Small output.** 7.7 kB per slide, 2.7 kB gzipped, and self-contained: theme
and layout are inlined and the built-in themes use system font stacks, so a
slide makes **zero network requests**. A test asserts that directly, because
"no CDN" is a promise that erodes one convenient import at a time.

A 500-slide deck emits **one** JavaScript file, shared by the presenter pages.
Audience slides ship none at all.

## What it looks like

Emitted by `vp build` in [examples/deck](./examples/deck), whose entire
configuration is `plugins: [slidx()]`. `node scripts/screenshot.mjs`
regenerates these, so an image that stopped being true fails to reproduce
rather than quietly misleading.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="./docs/images/2-dark.png">
  <img alt="A slide titled 'What actually goes wrong' listing five stage failures" src="./docs/images/2-light.png">
</picture>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="./docs/images/4-dark.png">
  <img alt="A slide with a table of linter rules and what each one catches" src="./docs/images/4-light.png">
</picture>

Both schemes come from one theme, because the room's lighting is usually
unknown until the day.

### The speaker's view

Ordered by how urgently a question needs answering mid-sentence, not by how
much space the answer needs. The clock is the largest thing on the page; the
notes get more room than the slide, because the slide is already on the wall
behind you and what you cannot see is what you meant to say about it.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="./docs/images/2-presenter-dark.png">
  <img alt="The presenter view: a large clock reading 0:00 of 20m, speaker notes, and a preview of the next slide" src="./docs/images/2-presenter-light.png">
</picture>

It counts past the slot rather than freezing at zero, warns three minutes out,
and keeps a second window on the same slide over a broadcast channel. Where
that channel is unavailable, mirroring is simply off and the deck still
presents.

## Why this exists

Making slides is the short part. Giving a talk is the long one, and almost
everything that goes wrong happens outside the editor:

- the venue Wi-Fi is down and the deck's fonts were on a CDN
- the body text was 18px and unreadable from row 12
- the projector washed out a colour pair that looked fine on a laptop
- the animation collapsed into one unreadable page in the exported PDF
- presenter view refused to open because the venue forced display mirroring
- the demo died and there was no fallback
- publishing afterwards was six manual steps, done exhausted, so it never happened

slidx treats those as build-time and runtime concerns of the framework, not as
the speaker's problem.

## Principles

**One model, one execution.** The editor, the projector, the presenter view,
the PDF, and the OG image all consume the same parsed deck. Presentation tools
fail when those quietly disagree; the only durable fix is to give them one
parser and one step engine.

**Steps are snapshots, not deltas.** Each stop on a slide is a _complete_ state,
compiled ahead of time. Advancing, going back, deep-linking to `?step=7`, and
printing all index into the same vector, so they cannot drift.

**Parsing never fails.** Decks get edited minutes before a talk. A bad line
produces a diagnostic and a slide that still renders — never an error that
leaves a speaker with nothing.

**Offline by default.** A built deck makes zero network requests. Fonts,
images, styles, and scripts are bundled. This is enforced at build time, not
documented as a best practice.

**The canvas and the file are one document.** Not import and export — two
views. Every gesture a presentation tool offers has a legible textual form, and
reading that form back reproduces the same visual state. Drag an image, colour
three words, add an animation: the diff is a line a reviewer can read, and
editing that line by hand moves the canvas.

That claim is mechanised, not asserted. Serialising is canonical, parsing
inverts it, and serialising is idempotent — checked as properties over
generated input, because a bidirectional tool that rewrites lines you did not
touch is one people stop trusting.

## Status

| Area                                                      | State    |
| --------------------------------------------------------- | -------- |
| `slidx_core` — deck model, parser, step pipeline          | building |
| `slidx_lint` — contrast, font size, overflow, a11y        | planned  |
| `slidx_render` — slide / presenter / print shells, themes | planned  |
| `@slidx/vite-plugin` — dev server, SSG, PDF, OG           | planned  |
| Visual editor                                             | planned  |
| Presenter suite — timer, notes, mirroring, A/V check      | planned  |
| Publish pipeline — PDF, Speaker Deck, OG, QR              | planned  |
| Integrations — Vue / React / Svelte / Angular / Three     | planned  |

See [ROADMAP.md](./ROADMAP.md) for the plan and the open issues for detail.

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

One DOM node whose text changes, not two that swap. Stepping backwards shows
the earlier value again, because each stop is a complete snapshot and the
runtime keeps no history.

For properties rather than content, say so in the timeline:

```md
---
steps:
  - set: { target: "#latency", color: success }
---
```

## Getting started

```bash
vp add -D @slidx/vite-plugin
```

```ts
// vite.config.ts
import { defineConfig } from "vite";
import { slidx } from "@slidx/vite-plugin";

export default defineConfig({
  plugins: [slidx()],
});
```

Nothing else is required: `slidx()` with no options finds `./slides`, serves the
visual editor in dev, and emits a static deck, a PDF, and OG images on build.

```bash
vp build
```

### The `slidx` command

Separate from the plugin, and optional. It does the two things a build cannot:
check the machine you are about to speak from, and fail a CI run on a deck that
will not survive the room.

```bash
curl -fsSL https://raw.githubusercontent.com/ubugeeei-prod/slidx/main/install.sh | sh
```

```bash
npm i -g slidx
```

Either way you get one prebuilt binary — no Node, no compiler, no `postinstall`
that downloads anything. The shell installer verifies the download against the
SHA-256 published with the release, installs under `~/.slidx/bin`, and never
asks for sudo. `--dry-run` shows what it would do first.

Prebuilt for macOS on Apple silicon and Intel, Linux on x86-64 and ARM64
(statically linked, so Alpine works too), and Windows on x86-64. Anywhere else,
`cargo install slidx_cli` builds it.

```bash
slidx doctor
```

Power, disk, clock, fonts, screen capture and network — the failures that never
happen at a desk. Worst first, each with what to do about it.

```bash
slidx lint
```

Every rule the build runs, over `./slides`, exiting non-zero on anything
blocking. That exit code is what makes it useful in CI.

`slidx` deliberately has no `build`: that is `@slidx/vite-plugin`'s job, and one
pipeline is the whole point.

## Development

[Vite+](https://voidzero.dev) runs every task in this repository, Rust and
TypeScript alike:

```bash
vp install
```

```bash
vp run workspace:ci
```

That last command is exactly what CI runs. `vp run` with no argument lists the
task graph; see [CONTRIBUTING.md](./CONTRIBUTING.md).

## License

MIT
