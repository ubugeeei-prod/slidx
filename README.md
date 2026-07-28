<p align="center">
  <strong>slidx</strong><br>
  <em>Slide + DX — a presentation framework for developers</em>
</p>

<p align="center">
  A Rust-powered, framework-agnostic toolkit that covers the whole life of a talk:<br>
  proposal, authoring, rehearsal, the room, the stage, and everything after it.
</p>

---

> [!NOTE]
> slidx is an independent personal project by [ubugeeei](https://github.com/ubugeeei), built on
> the [Ox Content](https://github.com/ubugeeei-prod/ox-content) Markdown engine.
> It is pre-alpha: the surface below is the target, not a changelog.

## Why this exists

Slide tools optimise for making slides. Giving a talk is a much longer job, and
almost everything that goes wrong happens outside the editor:

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
