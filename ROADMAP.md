# Roadmap

The ordering principle: **nothing ships that a speaker cannot rely on under
stage conditions.** Features land back-to-front — the parts that fail in a
conference room are built before the parts that look good in a demo.

Each milestone is independently useful. A user who stops at M2 has a better
tool than they had; a user who goes to M6 has one that covers the whole talk.

---

## M0 — Foundations

The deck model everything else consumes.

- [x] Cargo + pnpm workspace, CI, formatting and lint gates
- [x] `slidx_core`: fence-aware segmentation, frontmatter, notes, slugs
- [x] Declarative step pipeline compiled to full-state snapshots
- [x] Diagnostics that never fail a parse
- [ ] N-API bindings and a stable TypeScript deck type
- [ ] Snapshot and property tests over the parser

**Done when** a Markdown deck round-trips into a typed model with diagnostics,
from both Rust and Node, with no panics on adversarial input.

---

## M1 — Render and ship a deck

The minimum a speaker can stand on stage with.

- [ ] `slidx_render`: slide, presenter, and print shells
- [ ] Theme token system; `minimal`, `editorial`, `terminal`, `contrast` built in
- [ ] `@slidx/vite-plugin`: dev server, HMR, MPA static output
- [ ] Client runtime: navigation, step resolution, transitions, deep links
- [ ] **Offline guarantee**: build fails if any asset resolves to a remote host
- [ ] PDF export with each step as its own page
- [ ] OG image per slide and per deck

**Done when** `npm i -D @slidx/vite-plugin` → `vite build` produces a deck that
works with the network cable pulled.

---

## M2 — The linter

The failures that are invisible on a laptop and fatal on a projector.

- [ ] Contrast: WCAG ratio plus a projector model that simulates washout
- [ ] Minimum rendered font size, measured after theme scaling
- [ ] Content overflow and safe-area / caption-strip bleed
- [ ] Image resolution against the target render size; aspect distortion
- [ ] Missing alt text, heading order, colour-only encoding
- [ ] Animation cost: effects that will not stay on the compositor
- [ ] Time budget: per-slide budgets summed against the slot length
- [ ] `slidx doctor` — display resolution, fonts, network, DND, audio levels

**Done when** every documented stage failure has a rule that catches it before
the author leaves their desk.

---

## M3 — Visual editor

Editor-first authoring that still writes reviewable Markdown.

- [ ] Deck outline, slide canvas, inspector; all writes go back to Markdown
- [ ] Direct manipulation with snapping, guides, and layout tokens
- [ ] **Animation timeline** — the PowerPoint-shaped surface over `steps:`
- [ ] Live diagnostics from the linter, inline
- [ ] Media: video and audio embeds with level metering and loudness check
- [ ] Storyboard mode: edit at the level of one message per slide

**Done when** an author can build a staged, animated slide without typing YAML,
and the diff is still reviewable.

---

## M4 — The stage

Everything between walking up and sitting down.

- [ ] Presenter view: next slide, notes sized for a floor monitor, step preview
- [ ] Timer against the declared slot; behind/ahead indicator; optional-slide hints
- [ ] Mirroring and remote control across windows, screens, and devices
- [ ] Notification and Do-Not-Disturb control on entering presentation mode
- [ ] Rehearsal recording; actual per-slide dwell time diffed against budget
- [ ] **Demo fallback** as a declared construct: live target plus recorded video
- [ ] Audience channel — optional Cloudflare Worker for Q&A and reactions
- [ ] Live code sharing: publish a highlighted snippet, show its QR on screen

**Done when** a speaker can run the whole talk from slidx and recover from a
dead demo, a dead network, and a forced-mirroring projector.

---

## M5 — After the talk

The chore that is currently done exhausted, and therefore often not done.

- [ ] `slidx publish`: publication-grade PDF, embedded fonts, size checks
- [ ] Speaker Deck and Docswell upload with slug, description, and tags
- [ ] Social card, post text, and a blog scaffold generated from notes
- [ ] Resources page built from every link in the deck, with QR codes
- [ ] Attach the recording after the fact; archive and talk index

**Done when** publishing everywhere is one command driven by frontmatter the
author already wrote at proposal time.

---

## M6 — Integrations and reach

- [ ] Opt-in islands: Vue, React, Svelte, Angular, Three.js
- [ ] Theme packages distributable on npm
- [ ] Editor tooling: LSP, VS Code, Zed, Neovim
- [ ] Browser matrix: Chrome, Firefox, Safari, Edge — verified, not assumed
- [ ] Runtime matrix: Node, Bun, Deno; macOS, Linux, Windows

---

## Non-goals

- A proprietary binary deck format. The source stays Markdown.
- A hosted editor. slidx runs on the author's machine and in their CI.
- Framework lock-in. Every integration is opt-in and removable.
