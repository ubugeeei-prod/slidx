---
title: Lint rules
summary: What the linter checks, how severe each finding is, and how to suppress one.
section: reference
order: 5
---

# Lint rules

The linter is the part of slidx that models the room. It is not a style checker:
every rule here exists because of something that goes wrong on a stage and never
at a desk.

It runs in three places against the same rule set — inside `vite build`, from
`slidx lint`, and live in the editor and the language server — so a finding you
see while writing is the finding that will stop your build.

## The groups

Listed in the order the linter reports them, which is by how early in authoring
the problem is cheapest to fix: content shape first, then legibility, then the
presentation-day concerns.

<!-- slidx-docs: lint-groups -->

## How severe a finding is

| Severity  | What it means                                                  |
| --------- | -------------------------------------------------------------- |
| `error`   | Blocking. Stops `vite build`, and exits `slidx lint` non-zero. |
| `warning` | The deck renders, but not the way you probably meant.          |
| `info`    | Worth knowing, safe to ignore.                                 |

Only `error` blocks. `--strict` adds the advisory findings to a `slidx lint`
run, and `failOnDiagnostics: false` in the plugin's options lets a build finish
with a blocking one — the [night before](tonight.md#the-build-will-not-finish)
is the situation that exists for.

## Unchecked is not clean

One rule reports a third answer. `overflow-clipped` measures whether content
actually fits, which depends on where lines break, and no build-time model has
the font metrics to know that — so it is measured in the browser the build
already launches, and reports **unchecked** when there is none.

Unchecked and clean are opposite answers, and only one of them means the deck is
fine. Install the browser and the rule can speak:

```bash
vp exec playwright install chromium
```

The rest of the overflow group is arithmetic on numbers you declared — a safe
area, a venue's caption strip — so it runs everywhere, with no browser and no
guessing.

## Suppressing one

```bash
slidx lint --allow structure/too-many-bullets   # one code
slidx lint --allow structure                     # the whole group
```

A group name suppresses everything under it, and the boundary is the slash: a
prefix that stops in the middle of a segment suppresses nothing, so `--allow
struct` does not quietly turn off `structure`.

Before you suppress something, read what it said. Every diagnostic carries a
code, a position, and a concrete next action — a warning you cannot act on is
noise, and that is the standard the messages are written to.

## The offline rule is not adjustable in the same way

`offline/remote-asset` is what makes "a built deck needs no network" true rather
than aspirational. You can suppress it, and if you do, the deck you ship is one
that fetches something from a venue's Wi-Fi. That is a decision worth making
deliberately at a desk rather than discovering in a room.
