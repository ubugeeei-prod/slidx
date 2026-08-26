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

**Five boxes in M4 were unchecked for exactly this reason**, and the pattern is
one `check-dead-config.mjs` cannot see: a module in `packages/runtime` that no
shipped page imports. Presentation mode, the behind/ahead reading, the demo
switch, the phone remote and the whole audience channel were all written, all
tested, and all reachable by nobody. The equivalent check did not exist, and its
absence is why five features could sit in that state at once.

It exists now — `scripts/check-reachable.mjs`, which fails on a module no page
can call and reads import statements out of the Rust string literals that are
the runtime's real call sites. Its first run found four more nobody knew about,
and everything it still carries is listed in `UNREACHABLE` against the issue
that closes it. M7 is what came of that.

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
- [x] Remote control from a separate device — #13, #280. The author's
      Worker hosts a pairing session on `/sessions/<id>/socket`. The
      presenter mints the secret in the URL fragment, draws a QR, and the
      phone page sends positions. A deck that leaves the option out still
      fetches nothing. slidx does not hold the session.
- [x] Presentation mode: wake lock, fullscreen, and a named DND checklist —
      #13, #278. `f` on any slide takes the whole screen and asks for the wake
      lock, bound where that gesture has to be. The checklist is on the
      presenter view, which is the speaker's own screen and not the room's, and
      it opens the instant it is asked for rather than when a browser answers —
      the half no web API can do should not wait on the half one might refuse.
      The camera left `enterPresentation` for the window its tile is on — #296
- [x] Rehearsal recording; actual per-slide dwell time diffed against budget — #17
- [x] **Demo fallback** as a declared construct: live target plus recorded
      video — #14, #279. Both sides ship in the markup, so switching is one
      attribute write, and `d` performs it. The presenter says whether the
      recording will play, which only the projector can answer — its own
      preview is inert by design, and a page that fetched the same file would
      be answering about the wrong machine
- [x] Audience channel — moderated Q&A and reactions on a Worker — #16, #281.
      The Vite plugin always imports `@slidxjs/audience` (opt-in injects it; a
      barrel is not a door). `packages/audience/wrangler.toml` names
      `src/worker.ts` as `main`, which is a root `check-reachable` can see.
      A deck that leaves the option out still fetches nothing. The operator
      is the author's Cloudflare account; slidx holds no token and runs no
      relay
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

Everything above is checked except two boxes: a codec for images and fonts,
and the registry logins. v1.0 closes that — not more features, the ones
already paid for, connected to a hand. The pairing now reaches a slide.

- [x] `check:reachable`: CI fails on a module no page can call — #276
- [x] Pace reaches the presenter view — #277
- [x] Presentation mode: the checklist a browser cannot perform — #278
- [x] Demo fallback: the presenter knowing what has buffered — #279, #292
- [x] Remote control: a pairing that reaches a slide — #280
- [x] Audience channel: deployable, or a stated non-goal — #281
- [x] The editor's text controls, which nothing constructs — #283
- [x] The rehearsal trend across runs, which reaches no screen — #284
- [x] A staged slide answers neither a swipe nor `f` — #299
- [x] Two key tables, and the one that ships cannot be shown — #285
- [x] A clip's level, measured and shown to the presenter before the room
      hears it — #286
- [x] A declared camera that never opens — #296
- [x] An audience downloads 57% of a runtime it cannot run — #291
- [x] An image that does not move the slide, from a measurement already taken —
      #308
- [x] The image rules are silent in a build, and not from the CLI — #307
- [ ] Images and fonts: the half of the artefacts that needs a codec — #234
- [x] A toolchain that moves under the tree — #288
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
media feature that normalises a clip's level. Four more came from looking at
what the check had proved: an audience downloads 57% of a runtime it cannot
run, a floating toolchain turns somebody else's pull request red, two of a
deck's checked boxes are true of half its slides, and the image rules report
nothing in the build that most decks are checked by.

Every one of those is the same failure wearing a different coat: something
that cannot report is indistinguishable from something with nothing to
report.

What is left is not a list of unfinished features. Each of what remains is
waiting on something that is not code, and saying which is more useful than
an estimate:

**#281** has its answer: the author deploys the Worker from their Cloudflare
account. slidx writes the `wrangler.toml` and the client; it does not log in
and does not run a relay. **#280** (a pairing that reaches a slide) is the
same answer, now wired: the author's Worker hosts a second Durable Object
on `/sessions/<id>/socket`, the presenter mints a pairing in the URL
fragment, and the phone page sends positions. slidx still does not hold
the session.

**#286** is done without a build-time codec. The presenter page fetches the
next slide's file, `OfflineAudioContext.decodeAudioData` measures it, and
the audience slide writes the element's `volume` — Web Audio at presentation
time, not a native decoder in the build. **#234** (image formats and font
subsets) is still a file property that wants a codec that works offline, on
three platforms, without adding a native build step to a deck.

The half of #234 that needed no codec is done. Every image the plugin measures
now reaches the page with its own dimensions, so a browser reserves the box
before the file lands rather than reflowing the slide around it while a room is
reading — and the measurement was already being taken for the linter and thrown
away. Finding that also found #307: the linter and the renderer normalised a
reference two ways, so the image rules were silent in `vite build` and not from
`slidx lint`. `slidx_core::asset` is the one place now.

**The release** needs the maintainer signed in to two registries.
`RELEASING.md` is the sequence.

The milestone's own premise is closed either way. Nothing in the tree is
written, tested and reachable by nobody except what is listed above with its
reason, and there is now a check that fails when that stops being true.

---

## M8 — Keep making it, don't stop at the check

M7 closed the class of failure where a feature is written, tested, and
reachable by nobody. What is left of M7 is waiting on something that is not a
diff: a codec decision (#234), and two registry logins. #280 is wired:
the author's Worker is the relay. This milestone is the work that _is_
a diff — the documentation a new reader can actually start from, a Cloudflare
path that does not turn slidx into a service, motion an author can pick
rather than only declare, and a theme they can add after the fact.

GitHub issues for the new work belong here as closed-form tickets (why, done
when, the reachable path, what it will not do). They are written in this
file rather than only in the tracker, because a box that points at an issue
that does not exist is the same shape M7 was built to refuse. Existing open
issues are not duplicated: #1, #234, #275, #280.

A checked box still means a person can reach the thing.

### Documentation

- [x] **The published docs site is Ox Content 3 — #312.** Authored pages stay
      Markdown on GitHub. A prepare step fills the generated tables and
      rewrites the two link shapes that only work in the repository, then
      `@ox-content/vite-plugin` (3.0 alpha, pinned; `latest` is still 2.x)
      builds the HTML. `cargo test -p slidx_docs` still fails a dead link, a
      page in no section, or a placeholder naming a table nothing generates.
      Void still deploys `docs/dist`. The in-crate HTML shell remains for
      inspection and is not the published site. Brand tokens map onto
      `--octc-*`; radius stays 0; Ox Content's decorative skins are not used.
      _Won't:_ move the dead-link check into Vite; load Google fonts; generate
      an API reference nobody asked for.
- [x] **The front page says what it is, in sixty seconds.** One sentence:
      Markdown you write, a visual editor over the same file, static HTML a
      room can open with the network off. Sixty seconds is the install that
      will exist (`vp add -D @slidxjs/vite-plugin` → `plugins: [slidx()]` →
      `vp dev` / `/__slidx/`), labelled as unreleased so it does not pretend
      the package is on npm. The twenty-minute clone walkthrough is a
      separate page. Doors by need: write, look, present, hand out, islands,
      questions from the room, CLI. _Won't:_ a fake `npm i` that 404s; a stub
      page for a feature that has no reachable path yet (FrameScript, BGM).
- [x] **Japanese documentation.** Ox Content's locale map, same pages, not a
      second site with a second set of facts. The English site is the one a
      reader can start from (#312). `docs/content/ja/` is the same slugs;
      `cargo test -p slidx_docs` fails when a page exists in one locale and
      not the other. `i18n.enabled` names `en` and `ja`; `/` is English and
      `/ja/` is Japanese. Sidebar labels are locale maps. The header
      switcher is `ssg.localeSwitcher`.

### Publish and audience

- [x] **Cloudflare Pages, with no credential in slidx.** `slidx publish`
      grows a seventh destination. It writes the `wrangler.toml` / Pages
      fragment that belongs on disk and prints `wrangler pages deploy`. The
      author is logged into _their_ Cloudflare account; slidx still has no
      HTTP client and no token store. _Won't:_ slidx posting as you; a
      Pages project slidx owns; a CDN in front of the deck.
- [x] **Audience channel that a person can deploy — #281.** The Vite plugin
      always imports `@slidxjs/audience` (opt-in injects the client; a barrel
      is not a door). `packages/audience/wrangler.toml` names `src/worker.ts`
      as `main`, which is a root the reachable check can see. The docs page
      exists only because that path is green. The operator is the author's
      Cloudflare account. _Won't:_ a relay slidx runs. That would make this
      a service, which is a question about what slidx is, and the non-goals
      already refuse it. #280 is the same answer, now wired: the author's
      Worker hosts the pairing session. slidx does not hold it.

### Motion an author can actually pick

Twenty effect presets already compile (`fade`, `fly-in`, `wipe`, `zoom`,
`split`, `grow`, `float`, `typewriter`, `draw`, `pulse`, `shake`, `spin`,
and the rest) and the CSS for them already ships. The timeline writes
intent (`reveal` / `hide` / `emphasize`) and a named preset when the author
picks one. Timing and easing stay on the theme.

- [x] **The timeline cell offers the presets the compiler already has.**
      `StepPlacement` carries an optional preset. Selecting a cell, then a
      preset, writes a `SetStep` whose options name that preset and nothing
      else — timing and easing stay on the theme, which is the contract the
      timeline was compiled for. `vp run generate:types` updates the
      committed `deck.d.ts`. _Won't:_ a duration or an easing on the cell;
      a second motion model beside `EffectPreset`.
- [x] **Slide-to-slide motion stays the verbs MPA can do.**
      `none` / `fade` / `slide` / `push` / `wipe` / `rise` (`push-up`) are
      the ones a projector and `prefers-reduced-motion` can both survive.
      Wipe and rise degrade to a cross-fade under reduced motion, and to a
      cut in a browser that does not implement view transitions. Named
      view-transition elements for a figure that should keep its place
      across two documents are still ahead. _Won't:_ a client-side router;
      a spin or a zoom of the whole viewport; a promise that every browser
      plays the transition.
- [ ] **FrameScript.** A motion DSL the step compiler can read, that does
      not invade the easing the theme owns. Done when a timeline row and a
      Markdown fence name the same thing and both compile. _Won't:_ a page
      that describes a language nothing parses — that is a reachable-path
      failure wearing documentation's coat.
- [ ] **Three.js across two slides.** The island adapter already owns the
      loop and the GL context. A continuous scene is an extension of that
      ownership, not a second runtime that steals the canvas. _Won't:_ a
      WebGL context per slide that has to be thrown away and recreated as
      the browser follows a link.

### Themes after the fact

- [x] **`slidx theme add <pkg>`.** The four builtins stay four, because the
      projector audit is a closed set. `@slidxjs/theme-*` packages already
      exist. The command writes a `devDependency` or prints `vp add -D`;
      it does not fetch. `slidx theme` remains a leaf that lists builtins
      or audits a path — a positional `add` is a branch, not a subcommand
      that would steal a directory named `add`. The editor's theme picker
      reads installed packages, not a catalogue slidx hosts. _Won't:_ a
      network call from the binary; a fifth builtin.
- [ ] **`minimal` is the default people actually want, and extras are
      packages.** Stronger tokens on `minimal` (still radius 0, still no
      shadow, still no gradient). Gallery of `@slidxjs/theme-*` rather than
      a growing builtin list.

### Sound

- [ ] **BGM and SFX bound to a step, offline.** A clip is a file in the
      deck. Ducking and a doctor reading of the output level. The clip's
      own level now reaches the presenter before the room hears it — #286
      closed that half. Binding a clip to a step is the work that remains.
      _Won't:_ a stream from another origin; a deck that is silent until a
      CDN answers.

**Done when** a person who has never heard of slidx can tell what it is in
one page, start from a clone without being lied to about npm, publish a
deck to their own Cloudflare account, pick an effect from the timeline
they are looking at, and add a theme package without slidx growing a
network stack. Each of those is a path, not a module.

The order is the same one M7 learned: the documentation and the Cloudflare
handoff first (they are how a person reaches anything), then the timeline
preset (the surface that already exists and does not offer what it
compiles), then theme add, then FrameScript / named transitions / Three /
audio once each has a path that is not a stub.

---

## Where this stands

Everything checked above is merged, tested, and — where a browser can tell the
difference — verified in one rather than assumed. The counts are the honest
measure of that:

|                                          |                           |
| ---------------------------------------- | ------------------------- |
| Rust tests                               | 4054                      |
| TypeScript tests                         | 2118                      |
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

It had drifted again — 3825 and 1846 against a tree holding four thousand and
two thousand — and this time for a reason worth recording, because it is the
same shape as everything else in M7. The command that reproduces the table
**could not run**. It parses the test
runner's JSON report from the first `{` to the end of stdout, and the suites
under `scripts/` are node:test files whose ticks print around it; the moment one
landed after the report, the command threw. A count nobody can take is a count
that goes stale, and the only person who would notice is somebody already
editing this table.

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

The unchecked items are the work, not a wish list. Each M7 line is an open
issue with a stated shape. Each M8 line is the same shape written here, so a
box cannot point at a tracker entry that does not exist. Every unchecked line
above says _why_ it is not done rather than only that it is not.

## Non-goals

- A proprietary binary deck format. The source stays Markdown.
- A hosted editor. slidx runs on the author's machine and in their CI.
- Framework lock-in. Every integration is opt-in and removable.
- A credential store. Nothing here publishes as you.
