<p align="center">
  <strong>slidx</strong><br>
  <em>Slide + DX — the whole life of a talk, not just the slides</em>
</p>

---

> [!NOTE]
> An independent personal project by [ubugeeei](https://github.com/ubugeeei), built on the
> [Ox Content](https://github.com/ubugeeei-prod/ox-content) Markdown engine. Pre-alpha and
> **unreleased** — nothing is on npm or crates.io yet.

## The idea

**A talk is not a file. It is a project with a life.** You propose it, write it,
check it, stand up and give it, and publish it a week later. Almost everything
that goes wrong happens in the parts that are not writing.

So slidx is one toolchain across all of it, and every stage reads **one model
through one parser** — the editor, the projector, the PDF, the social card. They
cannot disagree about your deck, because there is nothing for them to disagree
with.

Two things follow, and they are what actually change day to day.

**Your deck is text, so every tool you already have works on it.** `git diff`
means something. A reviewer comments on a line. The visual editor writes
byte-range splices into the file you saved, so dragging a block produces a diff
like `{.side}` — one word — and your blank lines, your `*` bullets and your
hand-wrapped paragraphs are untouched. There is a test that drives eight edits
through a real repository and asserts the session removed _two_ lines.

**Your deck is pages, so the web works on it.** One HTML document per slide.
Navigation is a link. A slide can be bookmarked, indexed, opened on a phone, sent
to somebody, and printed — and it renders before any script runs, because there
is no script to run.

## The visual editor

Drag a block on the canvas. This is the entire diff:

```diff
+{.side}
 ![The four stages](./pipeline.svg)
```

Edit that line by hand and the canvas moves. **Not import and export — one
document, two views.** Colour three words and you get a mark; add an animation
and you get a `steps:` entry; move a block and you get a class naming a region
the theme owns. Never a pixel coordinate, so the placement still means the right
thing on a 4:3 projector and a reviewer can approve it in a diff.

While you drag, the linter runs — the same Rust the build runs, compiled to the
same WebAssembly module — so a column too narrow for the code you are dropping
into it says so before you let go.

## Three weeks out

```
slidx create vueconf-2026
```

Markdown, and a visual editor over the same file:

```md
---
title: Making Decks Fast
duration: 20m
theme: editorial
layout: aside
---

# Making Decks Fast

{.side}
![The four stages](./pipeline.svg)

Latency dropped to [120ms]{#latency}[38ms]{#latency}.

<!-- notes: Open with the outcome, not the agenda. -->
```

`[text]{#key}` names a range, so the canvas has somewhere to point when you
colour three words. Two marks sharing a key become **one element with successive
states** — `120ms` turns into `38ms` in place, rather than two that swap.
`{.side}` puts the block below it in the layout's `side` region: a placement a
reviewer can read, that still means the right thing on a 4:3 projector.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="./docs/images/2-dark.png">
  <img alt="A slide titled 'What actually goes wrong', listing five stage failures" src="./docs/images/2-light.png">
</picture>

## The week before

```
slidx lint
```

The linter checks **the room, not your monitor**: contrast through a model of
projector washout, font size by the angular size a glyph subtends from the back
row, overflow measured in a real browser per layout region, animations that will
not stay on the compositor, and your per-slide budgets summed against the slot
you were given.

## The night before

```
slidx doctor
```

This one checks **the machine you are about to speak from** — power, disk, clock
skew, the fonts your theme names, screen recording, display mirroring, Do Not
Disturb, audio levels. Worst first, each with what to do about it. Where a
platform gives no reliable reading it says so, because a guess is worse than
silence.

## On stage

A presenter view with the clock as the largest thing on the page, a
behind/ahead reading against the budgets you wrote, and the slides you marked
optional when you need to drop one. A shared code fence is published as its own
page inside your own output with a QR on the slide. A demo you declared as a
live target plus a recording, so a dead demo is one keypress.

And because a deck is pages: if your laptop dies, the talk is a URL away on
somebody else's browser.

## The week after

```
slidx publish
slidx export --target pdf   # browser | pdf | pdf-zip | png | pptx
```

Every destination is planned from the frontmatter you wrote at proposal time.
There is **no HTTP client and no token store** anywhere under it — a tool that
can post as you is a tool that has to be trusted with a credential — so it does
the half that needs no account and hands you the payload for the half that does.

## Across years

A speaker keeps five decks in five repositories and remembers where none of them
are.

```
slidx list                 # every deck this machine has seen
slidx grep "venue wifi"    # searches them all, and answers in slides, not line numbers
slidx cd vueconf           # with `slidx shell` loaded, takes you there
```

## Install

```bash
vp add -D @slidx/vite-plugin
```

```ts
// vite.config.ts
import { defineConfig } from "vite";
import { slidx } from "@slidx/vite-plugin";

export default defineConfig({ plugins: [slidx()] });
```

That is the whole configuration.

```bash
vp dev     # the deck, plus the visual editor at /__slidx/
vp build   # one HTML document per slide, asking nothing of anywhere else
```

The `slidx` binary is separate and optional — one prebuilt executable, no Node,
no compiler, no `postinstall` that downloads anything.

## Checked, not claimed

|                                                                                                                                                                                             |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Nothing from anywhere else.** Every request a built page makes is recorded in Chromium, Firefox and WebKit, and none of them leaves the deck's own files. A remote asset is a lint error. |
| **No framework in the output.** A slide wanting Vue or Three.js opts in alone.                                                                                                              |
| **Steps are snapshots.** Advancing, going back, `?step=7` and printing index one vector.                                                                                                    |
| **Parsing never fails.** A bad line is a diagnostic and a slide that still renders.                                                                                                         |
| **Native speed.** 500 slides in 133 ms — `node scripts/bench-build.mjs` reproduces it.                                                                                                      |

## More

**[Documentation](./docs)** — a walkthrough ending with a deck you built, a page
for deciding, and one indexed by symptom for the night before you speak.

**[ROADMAP.md](./ROADMAP.md)** — every unchecked line says _why_, and it opens
with what a checked box is allowed to mean. This project has found five features
that were built, tested, merged, and reachable by nobody; that section is the
result.

## License

MIT
