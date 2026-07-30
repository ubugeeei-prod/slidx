# Roadmap

The ordering principle: **nothing ships that a speaker cannot rely on under
stage conditions.** Features land back-to-front — the parts that fail in a
conference room are built before the parts that look good in a demo.

Tracking issue: #1

Each milestone is independently useful. A user who stops at M2 has a better
tool than they had; a user who goes to M6 has one that covers the whole talk.

## What a checked box means here

It means a person can reach the thing. That distinction was learned the hard
way, so it is written down rather than assumed.

`ShellOptions::include_runtime` was declared, defaulted, and documented as
"emitted into the page so the runtime can resolve steps". Nothing read it. The
compiled step pipeline — the feature most of `slidx_core` exists to produce —
therefore never ran on the one screen an audience looks at, while the PDF, the
print shell, and the presenter view all walked the stops correctly and every
test in the repository passed. The presenter view had the matching half of the
same gap: no navigation at all, on the window a clicker actually sends its keys
to.

Both are fixed, and `scripts/check-dead-config.mjs` now reports a public field
nothing reads — Rust's dead-code lint does not fire for a `pub` field in a
library, and this workspace's only callers are inside it. But the rule stands
on its own: a box here is checked when the path from a person to the behaviour
exists, not when the code that would implement it does.

---

## M0 — Foundations

The deck model everything else consumes.

- [x] Cargo + pnpm workspace, CI, formatting and lint gates
- [x] `slidx_core`: fence-aware segmentation, frontmatter, notes, slugs
- [x] Declarative step pipeline compiled to full-state snapshots
- [x] Diagnostics that never fail a parse
- [x] A stable TypeScript deck type, generated from the Rust model — #2
- [x] Snapshot and property tests over the parser

**Done when** a Markdown deck round-trips into a typed model with diagnostics,
from both Rust and Node, with no panics on adversarial input.

The bindings are WebAssembly rather than N-API, and the reason is not speed —
parsing a deck is microseconds either way. It is that **the same module runs in
the browser**, so the editor's live preview and the production build execute
identical code. A native addon would need a second implementation for the
editor, and a second implementation is a second set of answers.

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
- [x] Syntax highlighting, done while the deck is built — #5

**Done when** `npm i -D @slidx/vite-plugin` → `vite build` produces a deck that
works with the network cable pulled.

Highlighting happens in Rust at build time, which is what keeps the
zero-JavaScript claim true: a slide full of code ships coloured `<span>`s and no
highlighter.

Cross-document view transitions are Chromium's today. A deck that declares one
gets it there and an instant cut elsewhere — a degradation rather than a break,
but worth knowing before promising a room an animation.

---

## M2 — The linter

The failures that are invisible on a laptop and fatal on a projector.

- [x] Contrast: WCAG ratio plus a projector model that simulates washout — #7
- [x] Minimum rendered font size, by angular size at the back row — #7
- [x] Content overflow, measured in a real browser — #7
- [x] Safe-area and venue caption-strip bleed, from declared numbers — #7
- [x] Image resolution against the target render size; aspect distortion — #7
- [x] Missing alt text, heading order, bullet load, bare-URL links — #7
- [x] Animation cost: effects that will not stay on the compositor — #7
- [x] Time budget: per-slide budgets summed against the slot length — #7
- [x] `slidx doctor` — power, disk, clock, fonts, capture, network — #8
- [x] Image resolution reachable from `vite build` — the plugin reads the
      headers and hands over the sizes, because the WebAssembly boundary has no
      filesystem to read them itself — #7
- [ ] Doctor: display resolution, DND state, audio levels — no portable reading
      exists, and a guess about whether Do Not Disturb is on is worse than
      silence — #8

**Done when** every documented stage failure has a rule that catches it before
the author leaves their desk.

Overflow is split by what is knowable. A caption strip and a safe area are
_declared_ numbers, so they are arithmetic and run everywhere. Whether the
content fits depends on where lines break, which needs font metrics no
build-time model has — so that one is measured in the browser the build already
launches, and reports **unchecked** rather than clean when there is none.

---

## M3 — Visual editor

Editor-first authoring that still writes reviewable Markdown.

- [x] Edit operations as byte-range splices, never a re-serialisation — #9
- [x] Deck outline, slide canvas, inspector; all writes go back to Markdown — #9
- [x] Undo, as the inverse of an applied edit rather than a second document
- [x] Live diagnostics from the linter, inline
- [x] Media: video and audio embeds with level metering and loudness check — #11
- [ ] Direct manipulation with snapping, guides, and layout tokens
- [ ] **Animation timeline** — the PowerPoint-shaped surface over `steps:` — #10
- [ ] Storyboard mode: edit at the level of one message per slide

**Done when** an author can build a staged, animated slide without typing YAML,
and the diff is still reviewable.

The hard part was never the editor. It is that the file the editor writes has to
stay a file a human will review: parse to a model, mutate it, serialise back,
and the author's blank lines, their `*` bullets and their hand-wrapped paragraph
have all been regularised — invisible on the canvas and enormous in the diff. So
an operation is a **byte-range splice into the source the author saved**, and
the model exists only to compute the range.

That claim is held by a test that drives a real dev server against a real git
repository through eight edits and then asserts on `git diff`: four files
touched, **two** removed lines in the whole session, and not one of them a line
the author typed by hand.

The editor is served from `configureServer` and nowhere else, so a built deck
has no route that could write to an author's files. A test reads every emitted
page and fails if one mentions it.

---

## M4 — The stage

Everything between walking up and sitting down.

- [x] Presenter view: next slide, notes, position — #12
- [x] Timer against the declared slot, with a warning before the end
- [x] Behind/ahead indicator, and hints naming the optional slides to drop
- [x] Mirroring across windows and screens — #13
- [x] Navigation from the presenter view, which is where a clicker's keys go — #12
- [x] Remote control from a separate device — #13
- [x] Presentation mode: wake lock, fullscreen, and a named DND checklist — #13
- [x] Rehearsal recording; actual per-slide dwell time diffed against budget — #17
- [x] **Demo fallback** as a declared construct: live target plus recorded video — #14
- [x] Audience channel — moderated Q&A and reactions on a Worker — #16
- [x] Live code sharing: a highlighted snippet page, and its QR on the slide — #15
- [x] Snippet pages written by the build, so a scanned QR reaches a page
      rather than a 404 — #15

**Done when** a speaker can run the whole talk from slidx and recover from a
dead demo, a dead network, and a forced-mirroring projector.

A remote's secret travels in the URL fragment, which is not sent with the
request and therefore reaches no access log, no referrer header, and no proxy. A
pairing URL arriving with the secret in its _query_ is refused rather than
honoured: that URL has already been written down somewhere.

The demo fallback ships both sides in the markup, so switching is one attribute
write. A fallback that has to be fetched when the demo dies is not a fallback —
it is a second thing that fails, at the same moment, for the same reason.

---

## M5 — After the talk

The chore that is currently done exhausted, and therefore often not done.

- [x] Publish planning, payloads, and caps for every target — #18
- [x] `slidx publish`, performing everything that needs no account — #18
- [x] Speaker Deck and Docswell payloads with slug, description, and tags — #18
- [x] Social card, post text, and a blog scaffold generated from notes
- [x] Resources page built from every link in the deck — #19
- [x] QR encoder, and QR codes rendered onto slides — #19
- [x] Attach the recording after the fact; archive record and talk index — #20

**Done when** publishing everywhere is one command driven by frontmatter the
author already wrote at proposal time.

There is no HTTP client under `slidx publish` and no token store, and that is a
property rather than an omission: a tool that can post as you is a tool that has
to be trusted with a credential. It composes the payload, writes what belongs on
disk, and names the page to paste the rest into.

The archive record is the one target whose input is not finished when it first
runs, so it separates **blocked** — a field the author can add now — from
**pending**, a field the world has not produced yet. Nobody can make a
conference publish a video.

---

## M6 — Integrations and reach

- [x] Opt-in islands: Vue, React, Svelte, Three.js, Angular — #21
- [x] Language server: diagnostics, completion, outline, hover — #23
- [x] Browser matrix: Chromium, Firefox, WebKit — verified, not assumed — #23
- [x] Runtime matrix: Node, Bun, Deno; macOS, Linux, Windows — #23
- [x] The `slidx` binary, by `curl | sh` or `npm i -g`, with published checksums — #23
- [x] Version manager, deck index, fuzzy finder, `preview`, completions, TUI — #23
- [x] `list`, `grep` and `cd` over the decks the index already knows — #23
- [ ] Theme packages distributable on npm — #3
- [ ] Editor plugins for VS Code, Zed, and Neovim over the language server — #23
- [ ] Documentation site
- [ ] First release to npm and crates.io — needs the maintainer's accounts

Angular cost more than this roadmap assumed. Its published packages ship
partially compiled and will not evaluate without its own linker, so a deck using
an Angular island needs an Angular plugin in its own Vite config. Nothing in
`@slidx/*` grew a dependency and the plugin belongs to the deck's author — but
it is a configuration step the other four do not have.

The terminal preview renders the deck's _model_, never its HTML, and says so on
every frame: it previews structure and flow, not appearance. Nothing about a
character grid can tell you whether type is legible from row 15. That belongs to
the linter and to a real browser, and both already exist.

`slidx cd` prints a path instead of entering one, and no release will change
that. A child process cannot move the shell that started it — `chdir` acts on
the caller — so the command resolves and a shell function enters. Every
directory jumper that looks like it does otherwise is that same pair.

`slidx grep` reports the slide a match is on rather than the line of a file,
which is what makes it worth having over `grep -r`: `slides/0007.md:12` is an
address a speaker cannot use, and "slide 7 of the VueConf deck" is one they can.
It reads only decks and stops at `node_modules`, build output and dot
directories, and it parses a deck only once something in it has already matched
— so a search that finds nothing costs one read per deck.

---

## Where this stands

Everything checked above is merged, tested, and — where a browser can tell the
difference — verified in one rather than assumed. The counts are the honest
measure of that:

|                                     |                           |
| ----------------------------------- | ------------------------- |
| Rust tests                          | 1606                      |
| TypeScript tests                    | 1046                      |
| Crates                              | 10                        |
| Published packages                  | 8                         |
| Platforms in CI                     | Linux, macOS, Windows     |
| Browsers exercised                  | Chromium, Firefox, WebKit |
| Runtimes exercised                  | Node, Bun, Deno           |
| JavaScript on a slide with no steps | none                      |

That last row used to read "on an audience slide", and the correction is the
point of this document. A slide with steps loads one shared module and its own
compiled timeline; a slide without steps is finished markup and loads nothing.
The old wording was true only because the feature did not work.

The unchecked items are the work, not a wish list. Each is an open issue with a
stated shape, and each unchecked line above says _why_ it is not done rather
than only that it is not.

## Non-goals

- A proprietary binary deck format. The source stays Markdown.
- A hosted editor. slidx runs on the author's machine and in their CI.
- Framework lock-in. Every integration is opt-in and removable.
- A credential store. Nothing here publishes as you.
