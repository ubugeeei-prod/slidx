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

**Five boxes in M4 have since been unchecked for exactly this reason**, and the
pattern is one `check-dead-config.mjs` cannot see: a TypeScript symbol exported
from `packages/runtime`'s barrel that no shipped page imports. Presentation
mode, the behind/ahead reading, the demo switch, the phone remote and the whole
audience channel are all written, all tested, and all reachable by nobody. The
equivalent check — an exported symbol with no consumer — does not exist, and its
absence is why five features could sit in that state at once.

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
- [x] `@slidxjs/vite-plugin`: dev server, live reload, MPA static output — #4
- [x] Client runtime: step resolution and the anchor contract — #4
- [x] Client runtime: navigation, keyboard, deep links — #4
- [x] Slide-to-slide transitions — #4
- [x] `slidx self-update`, checksum-verified and install-channel aware
- [x] **Offline guarantee**: a remote asset is a lint error — #5
- [x] Print shell with each stop as its own page — #6
- [x] Automated PDF at build time — #6
- [x] OG image per slide and per deck
- [x] Syntax highlighting, done while the deck is built — #5

**Done when** `npm i -D @slidxjs/vite-plugin` → `vite build` produces a deck that
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
- [x] Doctor: display arrangement and mirroring, Do Not Disturb, audio levels —
      read natively per platform. The line above used to say no portable reading
      exists; that was an argument about the browser, and `slidx` is a binary the
      speaker installed. A guess is still worse than silence, so a platform that
      will not answer is reported as unknown with the reason — #8

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
- [x] Direct manipulation: drag a block between the layout's regions, with
      guides for boundaries, the safe area and the other blocks' edges — and the
      linter warning while it is still moving. Never a coordinate: dropping into
      the default region is written by _removing_ the class, so a drag out and
      back is byte-identical
- [x] **Animation timeline** — rows are what a slide addresses, columns are its
      stops, and the playhead scrubs — #10
- [x] Storyboard mode: one message per slide, each drawn as wide as its time
      against the slot, with the optional slides shown as the slack they are
- [x] **Two people in one deck.** The dev server holds the one document, so a
      change from the canvas and a file the author saved in their own editor
      merge rather than overwrite. Presence names the block each person has
      selected, not only the slide, and the canvas draws a mark on it with
      their name — because knowing somebody is on slide four does not stop two
      people rewriting the same paragraph
- [x] Follow another editor: press a name in the roster and move with them
      until you select something yourself, they close their tab, or you press
      it again

**Done when** an author can build a staged, animated slide without typing YAML,
and the diff is still reviewable.

A mark is a claim that a named person is working on a named block, so the
interesting half of that feature is what it refuses to draw. A viewer with no
block selected gets nothing, and so does one whose block is not on this slide —
their position and the deck it is a position in arrive on the same stream but
not in the same frame, which is a real second of every move somebody makes. A
rectangle guessed for them would carry a name.

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

The timeline is the surface that model was compiled for. Because every stop is a
complete snapshot rather than a delta, its playhead **scrubs**: dragging left
costs what dragging right costs, nothing is replayed, and there is no animation
state to run forward. A cell holds the author's intent — `reveal`, `hide`,
`emphasize`, `set` — and never a duration or an easing, because motion belongs to
the theme and an editor that wrote timing onto every action it touched would
move that decision into the deck one click at a time.

`autoSteps:` is a one-way door, and the timeline is where that is visible rather
than discovered. Generated stops have no line in the file, so they are shown,
named as generated, and refuse to be clicked — with one action offered that
writes them out as an explicit `steps:` list. That action leaves `autoSteps:`
where it is, because the mode is what puts the anchors the written steps name
into the markup.

---

## M4 — The stage

Everything between walking up and sitting down.

- [x] Presenter view: next slide, notes, position — #12
- [x] Timer against the declared slot, with a warning before the end
- [x] Behind/ahead indicator, and hints naming the optional slides to drop —
      #277. One line under the presenter's clock, and every word of it comes
      from `describePace` rather than from a second opinion written beside it
- [x] Mirroring across windows and screens — #13
- [x] Navigation from the presenter view, which is where a clicker's keys go — #12
- [ ] Remote control from a separate device — #13. `createPairing`,
      `pairingUrl` and `createRemoteTransport` exist and nothing constructs a
      `RemoteSocket` for them; there is no relay. `readPairing`'s one caller
      is the _editor's_ collaboration gate, not slide control
- [x] Presentation mode: wake lock, fullscreen, and a named DND checklist —
      #13, #278. `f` on any slide takes the whole screen and asks for the wake
      lock, bound where that gesture has to be. The checklist is on the
      presenter view, which is the speaker's own screen and not the room's, and
      it opens the instant it is asked for rather than when a browser answers —
      the half no web API can do should not wait on the half one might refuse.
      `enterPresentation` also arranges a camera and a camera still never
      opens: the tile is on the slide and the ask is from the lectern — #296
- [x] Rehearsal recording; actual per-slide dwell time diffed against budget — #17
- [ ] **Demo fallback** as a declared construct: live target plus recorded
      video — #14. Both sides ship in the markup, which is the hard half and
      is done. Switching between them is one attribute write and nothing a
      speaker can reach performs it: `createDemoSwitch` has no caller
- [ ] Audience channel — moderated Q&A and reactions on a Worker — #16.
      `@slidxjs/audience` is 1,983 lines with its own protocol, room state and
      rate limiting, no `package.json` in the workspace depends on it, and
      there is no wrangler configuration to deploy the Worker anywhere
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

**Mirroring only ever worked in one direction, and the box was checked anyway.**
Two windows, two counters, and one watermark between them: the mirror ordered
messages by a sequence documented as monotonic _per sender_ and compared it
against a single highest-seen. A deck is multi-page HTML, so every move reloads
a window and restarts its counter at one — and the presenter view announces its
position on load, which raised that bar to 1 before anything happened. From then
on it dropped every position the projector sent, because a freshly loaded
projector page can only count to 1 as well.

The speaker drives from the projector, because that is where a clicker's keys
land. Their notes stopped following, silently, for the whole talk. Every test
passed: with one sender the rule is correct, and every test had one sender.
`MirrorMessage.from` is the fix and the three tests beside it are named after
the failure.

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
- [x] `slidx export`: the static site, the deck as one document, one PDF per
      slide, one image per stop — reachable for all four from the command line
- [x] `slidx export --target pptx`, for an OOXML presentation — a rendered
      image per stop plus the notes as real notes text, so what a speaker has to
      keep editing survives the trip

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

`slidx export` had to be reconciled with the promise that slidx has no `build`
before a line of it was written, because an export that rendered a deck a second
way would break exactly that. So it **runs the deck's own build and packages what
that wrote** — the plugin, the print shell, one browser pass — and produces
nothing at all without the plugin installed. The frames a build does not
ordinarily write, a document per slide and an image per stop, are rendered by
that same pass because it is the one place the answer can come from.

It runs `node_modules/.bin/vite` rather than a package manager, and the reason is
the offline guarantee: `npm exec` installs what it cannot find, so a command that
went through one could reach a registry mid-export on the machine where that
matters least — a laptop the night before, on conference wifi.

---

## M6 — Integrations and reach

- [x] Opt-in islands: Vue, React, Svelte, Solid, Three.js, Angular — #21
- [x] Opt-in MDX component syntax, compiled to static-first islands without
      evaluating deck source
- [x] Language server: diagnostics, completion, outline, hover — #23
- [x] Browser matrix: Chromium, Firefox, WebKit — verified, not assumed — #23
- [x] Runtime matrix: Node, Bun, Deno; macOS, Linux, Windows — #23
- [x] The `slidx` binary, by `curl | sh` or `npm i -g`, with published checksums — #23
- [x] Version manager, deck index, fuzzy finder, `preview`, completions, TUI — #23
- [x] `list`, `grep` and `cd` over the decks the index already knows — #23
- [x] Theme packages distributable on npm — a token document rather than a
      stylesheet, so the linter can still check it, and a built-in always wins a
      name so no dependency can quietly redefine `theme: minimal` — #3
- [x] Editor plugins for VS Code, Zed, and Neovim over the language server. The
      server itself reached nobody first: `release.yml` built `--bin slidx` and
      nothing else, so `slidx-lsp` was on no machine that installed slidx. It is
      `slidx lsp` now — #23
- [x] Documentation site, with the sections built around readers rather than crates
- [ ] First release to npm and crates.io — needs the maintainer's accounts

Everything either side of that box in M6 is done. `vp run release <level>` writes the
version everywhere it lives, runs the version check against the tag it is about
to create, and pushes it; the tag starts a workflow that publishes through OIDC
with no token stored anywhere. A dry run over all 28 publishable directories is
what found the two things now fixed: every tarball carried `"license": "MIT"`
and none carried the notice, and 26 of them would have published to a blank
page.

What cannot be done from here is the part that is not reversible. Claiming
`slidx` and `@slidxjs/*` needs the maintainer signed in, and trusted publishing
has to be configured on both registries before a tag can use it —
`RELEASING.md` is the sequence. Nothing on either registry answers to these
names yet, which is the only reason there is still time to be careful about it.

Angular cost more than this roadmap assumed. Its published packages ship
partially compiled and will not evaluate without its own linker, so a deck using
an Angular island needs an Angular plugin in its own Vite config. Nothing in
`@slidxjs/*` grew a dependency and the plugin belongs to the deck's author — but
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

## M7 — v1.0

Tracking issue: #275

Everything above is checked except six boxes, and five of those six are the
same defect wearing five faces: the code is written, the tests pass, and no
person can reach it. v1.0 closes that — not more features, the ones already
paid for, connected to a hand.

- [x] `check:reachable`: CI fails on a module no page can call — #276
- [x] Pace reaches the presenter view — #277
- [x] Presentation mode: the checklist a browser cannot perform — #278
- [ ] Demo fallback: a key that performs the one attribute write — #279
- [ ] Remote control: a pairing that reaches a slide — #280
- [ ] Audience channel: deployable, or a stated non-goal — #281
- [x] The editor's text controls, which nothing constructs — #283
- [ ] The rehearsal trend across runs, which reaches no screen — #284
- [x] A staged slide answers neither a swipe nor `f` — #299
- [x] Two key tables, and the one that ships cannot be shown — #285
- [ ] A clip's level, measured and shown to nobody — #286
- [ ] A declared camera that never opens — #296
- [x] An audience downloads 57% of a runtime it cannot run — #291
- [ ] Images and fonts: the artefact half of performance — #234
- [ ] A toolchain that moves under the tree — #288
- [ ] First release to npm and crates.io — needs the maintainer's accounts

**Done when** every promise the README makes is reachable by a person, measured
rather than asserted, and installable.

The order is not arbitrary. The check comes first because it is the acceptance
criterion for most of what follows it: each of those closes when the check
stops reporting its module, which is a stronger statement than a reviewer
reading a diff and believing it. Closing five unreachable features by hand
without building the thing that looks for the sixth leaves the sixth to be
found by a speaker, on a stage — and there turned out to be nine.

`check-dead-config.mjs` catches the shape that produced
`ShellOptions::include_runtime` — a `pub` field nothing reads. It cannot catch
the shape that produced these five, because a symbol's real call site in this
repository is a string literal in another language: `slidx_render` emits
`import { … } from "…"` into the page it renders. An off-the-shelf dead-export
tool reads that file as Rust, sees no import, and reports the entire runtime as
unused and every editor module as used — exactly backwards.

The list grew on the day the check first ran, which is the argument for having
built it first. Four modules nobody knew were unreachable came out of one
scan — an editor surface with no constructor call anywhere in the workspace,
the rehearsal comparison across runs, a second key table, and the half of the
media feature that normalises a clip's level. Two more findings came from
looking at what the check had proved: an audience downloads 57% of a runtime
it cannot run, and a floating toolchain turns somebody else's pull request
red.

Two of the fourteen cannot be finished from here, and they are named rather
than quietly carried. The registries need the maintainer signed in; #281 needs
a decision about who operates a Worker, which is a question about what slidx
_is_ rather than about what it does.

---

## Where this stands

Everything checked above is merged, tested, and — where a browser can tell the
difference — verified in one rather than assumed. The counts are the honest
measure of that:

|                                          |                           |
| ---------------------------------------- | ------------------------- |
| Rust tests                               | 3825                      |
| TypeScript tests                         | 1846                      |
| Crates                                   | 19                        |
| Publishable npm packages                 | 10                        |
| Platforms in CI                          | Linux, macOS, Windows     |
| Browsers exercised                       | Chromium, Firefox, WebKit |
| Runtimes exercised                       | Node, Bun, Deno           |
| JavaScript a slide with no steps fetches | none                      |

`node scripts/count-coverage.mjs` reproduces the first four rows, so they are
measured rather than remembered. They had said 1642, 1080 and 10 against a tree
holding 2316, 1157 and 13 — written by hand, true on the afternoon they were
typed, and drifted far enough that two readers noticed independently before
anyone corrected them. A table that calls itself the honest measure has to be
able to prove it.

That last row has now been corrected twice, and both corrections are the point
of this document.

It first read "on an audience slide", which was true only because the compiled
step pipeline reached no projector. A slide with steps loads one shared module
and its own compiled timeline; a slide without steps fetches nothing.

Then it read "on a slide with no steps: none", and that was true for a worse
reason: **such a slide could not be advanced.** No key was bound, because the
key handler shipped with the stage. Nothing in the body linked anywhere —
`<link rel="next">` sat in the `<head>`, where no browser has surfaced it for
twenty years. And the presenter view's mirror broadcast into a window with no
listener, so a speaker on the title slide could press the clicker and watch the
projector not move. The runtime had `next`, `prev`, `first`, `last` and a key
table the whole time. Nobody in a room could reach any of them.

A deck you cannot advance is not a deck, so navigation is now on every slide:
two real anchors in the footer, which cost nothing and work from a USB stick
with the script disabled, and a few hundred inline bytes that turn a clicker
and the presenter's mirror into a click on one of them. `slidx_render::navigation`
has the reasoning; `scripts/budget.mjs` holds the size and keeps the half of the
old claim that was ever load-bearing — a finished slide still **fetches**
nothing.

The unchecked items are the work, not a wish list. Each is an open issue with a
stated shape, and each unchecked line above says _why_ it is not done rather
than only that it is not.

## Non-goals

- A proprietary binary deck format. The source stays Markdown.
- A hosted editor. slidx runs on the author's machine and in their CI.
- Framework lock-in. Every integration is opt-in and removable.
- A credential store. Nothing here publishes as you.
