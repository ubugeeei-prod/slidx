---
title: Steps and animation
summary: The three ways to stage a slide, and the one model all of them compile to.
section: reference
order: 4
---

# Steps and animation

A slide can have more than one state. slidx calls each of those a **stop**, and
a stop is a _complete_ snapshot rather than a delta — advancing, going back,
deep-linking to `?step=7` and printing all index into the same vector, so they
cannot drift apart.

That is why the animation you author is the animation that prints. The
projector, the presenter view, the PDF and the social card consume the same
compiled timeline.

## Three ways to author, one thing they produce

### A marker in the prose

```md
- Build time fell to 28ms <!-- step -->
- The PDF stopped losing the animation <!-- step -->
```

The light form, and the common case. A marker at the end of a bullet stages that
bullet; a marker alone on a line stages the block above it, which is how a code
fence, a table or an image gets staged.

A marker may name a preset: `<!-- step: fly-in -->`. An unrecognised name is a
diagnostic and the marker still becomes a stop, because a typo the night before
a talk should cost you an effect and not a slide.

### An automatic mode

```md
---
autoSteps: list
---
```

Stages every top-level item without a marker anywhere. Set it on a slide, or on
the deck's first block to make it the default and `autoSteps: none` on the one
slide that should arrive whole.

<!-- slidx-docs: auto-steps -->

### An explicit list

```md
---
steps:
  - reveal: ".finding"
  - emphasize: { target: "#result", preset: pulse }
  - set: { target: "#latency", color: success }
  - group:
      - reveal: ".left"
      - reveal: ".right"
---
```

Five verbs. `reveal` brings an element on, `hide` takes it off, `emphasize`
draws attention to something already there, `set` changes an element in place,
and `group` puts several intents on one click. `emphasise` is accepted as well,
because two spellings of one word is not a thing to be strict about.

Each takes `preset:`, `duration:` in milliseconds, `after:` in milliseconds for
a stop that plays itself rather than waiting, and `from:` for the presets that
travel.

## The presets

The **phase** column says when a preset belongs: an entrance, an emphasis, or an
exit. The cost is not decoration — the `motion` rule flags a slide whose effects
will not stay on the compositor, and this is the column that decides whether it
fires.

<!-- slidx-docs: step-presets -->

## Addressing part of a line

A _mark_ names a range inside a block. It is the smallest thing the visual
editor can point at, and the reason that selecting three words and colouring
them has somewhere to go in the file:

```md
The result was [3.2x faster]{#result .accent}.
```

## Changing something already on screen

Revealing covers "not there yet". For a value that _changes_, write the takes
next to each other and they become one element with successive states:

```md
Latency dropped to [120ms]{#latency}[38ms]{#latency}.
```

One DOM node whose text changes, not two that swap. Stepping backwards shows the
earlier value again, because each stop is a complete snapshot and the runtime
keeps no history.

For a property rather than content, say so in the timeline with `set:`.

## What a marker leaves behind

Every marker compiles to an empty `<span data-slidx-step="N" hidden>` in the
Markdown. It survives any Markdown renderer, which is what keeps slidx
framework-agnostic: a Vue island, a React island and a plain slide all end up
with the same anchor in the same place, and the runtime resolves the staged
element from it by one rule implemented identically in the client and in the
print renderer.

The [module's own documentation](../../crates/slidx_core/src/markers.rs) states
that rule in three cases, and the renderer asserts each of them.
