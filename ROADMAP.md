# Roadmap

The ordering principle: **nothing ships that a speaker cannot rely on under
stage conditions.** Features land back-to-front — the parts that fail in a
conference room are built before the parts that look good in a demo.

Tracking issue: #1

Each milestone is independently useful. A user who stops at M2 has a better
tool than they had; a user who goes to M6 has one that covers the whole talk.

---

## M0 — Foundations

The deck model everything else consumes.

- [x] Cargo + pnpm workspace, CI, formatting and lint gates
- [x] `slidx_core`: fence-aware segmentation, frontmatter, notes, slugs
- [x] Declarative step pipeline compiled to full-state snapshots
- [x] Diagnostics that never fail a parse
- [ ] N-API bindings and a stable TypeScript deck type — #2
- [ ] Snapshot and property tests over the parser

**Done when** a Markdown deck round-trips into a typed model with diagnostics,
from both Rust and Node, with no panics on adversarial input.

---

## M1 — Render and ship a deck

The minimum a speaker can stand on stage with.

- [x] `slidx_render`: slide, presenter, and print shells — #3
- [x] Theme token system; `minimal`, `editorial`, `terminal`, `contrast` built in — #3
- [x] `@slidx/vite-plugin`: dev server, live reload, MPA static output — #4
- [x] Client runtime: step resolution and the anchor contract — #4
- [x] Client runtime: navigation, keyboard, deep links — #4
- [x] Slide-to-slide transitions — #4
- [x] **Offline guarantee**: a remote asset is a lint error — #5
- [x] Print shell with each stop as its own page — #6
- [x] Automated PDF at build time — #6
- [x] OG image per slide and per deck

**Done when** `npm i -D @slidx/vite-plugin` → `vite build` produces a deck that
works with the network cable pulled.

---

## M2 — The linter

The failures that are invisible on a laptop and fatal on a projector.

- [x] Contrast: WCAG ratio plus a projector model that simulates washout — #7
- [x] Minimum rendered font size, by angular size at the back row — #7
- [ ] Content overflow and safe-area / caption-strip bleed — #7 (needs layout measurement)
- [ ] Image resolution against the target render size; aspect distortion — #7
- [x] Missing alt text, heading order, bullet load, bare-URL links — #7
- [x] Animation cost: effects that will not stay on the compositor — #7
- [x] Time budget: per-slide budgets summed against the slot length — #7
- [x] `slidx doctor` — power, disk, clock, fonts, capture, network — #8
- [ ] Doctor: display resolution, DND state, audio levels (no std-only reading exists) — #8

**Done when** every documented stage failure has a rule that catches it before
the author leaves their desk.

---

## M3 — Visual editor

Editor-first authoring that still writes reviewable Markdown.

- [ ] Deck outline, slide canvas, inspector; all writes go back to Markdown — #9
- [ ] Direct manipulation with snapping, guides, and layout tokens
- [ ] **Animation timeline** — the PowerPoint-shaped surface over `steps:` — #10
- [ ] Live diagnostics from the linter, inline
- [ ] Media: video and audio embeds with level metering and loudness check — #11
- [ ] Storyboard mode: edit at the level of one message per slide

**Done when** an author can build a staged, animated slide without typing YAML,
and the diff is still reviewable.

---

## M4 — The stage

Everything between walking up and sitting down.

- [x] Presenter view: next slide, notes, position — #12
- [x] Timer against the declared slot, with a warning before the end
- [ ] Behind/ahead indicator and optional-slide hints
- [x] Mirroring across windows and screens — #13
- [ ] Remote control from a separate device — #13
- [x] Presentation mode: wake lock, fullscreen, and a named DND checklist — #13
- [ ] Rehearsal recording; actual per-slide dwell time diffed against budget — #17
- [ ] **Demo fallback** as a declared construct: live target plus recorded video — #14
- [x] Audience channel — moderated Q&A and reactions on a Worker — #16
- [ ] Live code sharing: publish a highlighted snippet, show its QR on screen — #15

**Done when** a speaker can run the whole talk from slidx and recover from a
dead demo, a dead network, and a forced-mirroring projector.

---

## M5 — After the talk

The chore that is currently done exhausted, and therefore often not done.

- [x] Publish planning, payloads, and caps for every target — #18
- [ ] `slidx publish` CLI that performs a plan — #18
- [x] Speaker Deck and Docswell payloads with slug, description, and tags — #18
- [x] Social card, post text, and a blog scaffold generated from notes
- [x] Resources page built from every link in the deck — #19
- [x] QR encoder — #19
- [x] QR codes rendered onto slides — #19
- [ ] Attach the recording after the fact; archive and talk index — #20

**Done when** publishing everywhere is one command driven by frontmatter the
author already wrote at proposal time.

---

## M6 — Integrations and reach

- [x] Opt-in islands: Vue, React, Svelte, Three.js — #21
- [ ] Angular island (needs its compiler and a change-detection provider) — #21
- [ ] Theme packages distributable on npm — #3
- [ ] Editor tooling: LSP, VS Code, Zed, Neovim — #23
- [ ] Browser matrix: Chrome, Firefox, Safari, Edge — verified, not assumed — #23
- [ ] Runtime matrix: Node, Bun, Deno; macOS, Linux, Windows — #23

---

## Where this stands

Everything checked above is merged, tested, and verified in a browser rather
than assumed. The counts are the honest measure of that:

|                                 |                       |
| ------------------------------- | --------------------- |
| Rust tests                      | 408                   |
| TypeScript tests                | 212                   |
| Platforms in CI                 | Linux, macOS, Windows |
| JavaScript on an audience slide | none                  |

The unchecked items are the work, not a wish list — each one is an open issue
with a stated shape.

## Non-goals

- A proprietary binary deck format. The source stays Markdown.
- A hosted editor. slidx runs on the author's machine and in their CI.
- Framework lock-in. Every integration is opt-in and removable.
