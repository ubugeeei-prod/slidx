# The slidx brand

Beautiful, simple, geometric. No shadows, no gradients.

That is a brief, and this document is mostly about where each part of it became
a check instead of a paragraph. A brand is the one part of a project that
normally escapes the repository's own standards and lives in a PDF nobody
opens. This one does not escape: the colours go through slidx's linter, the
flatness is a CI gate, and every file under `assets/brand/` is generated from
Rust and compared against its committed copy.

## Where the tokens live

```
assets/brand/tokens.json
```

That is the contract. Both a TypeScript build and a hand-written page can read
it, it has no build step in front of it, and a change to the brand arrives in
review as a readable diff. `assets/brand/tokens.css` is the same values as
custom properties, for a page that would rather link one file than transform
JSON.

Everything in `assets/brand/` is generated:

```bash
vp run generate:brand
```

`slidx_brand::assets` compares each committed file against what the crate
generates now, so a colour changed in Rust and not regenerated fails
`vp run workspace:ci` rather than leaving a consumer drawing last month's
brand. Nothing here was exported from a design tool, and there is no step in
which somebody re-crops a PNG by hand.

## The mark

![The slidx mark](../assets/brand/mark-light.svg)

**What it is about.** slidx compiles one document into a sequence of pages.
That is the product in one sentence, and it is what the mark draws: a single
full-height block, a gutter, and a run of pages beside it that add up to
_exactly_ the same height. One form or many — the same document either way.

The colour does the other half of the sentence. The document is **ink**,
because the author writes it. The pages are **signal**, because slidx produces
them. Reversed, the mark would be arguing the opposite.

**The construction.** A 24-unit square on a module of **3**, so the grid is
8 × 8 modules and every edge lands on it.

| part         | modules | units         |
| ------------ | ------- | ------------- |
| the document | 3 × 8   | x 0…9, y 0…24 |
| the gutter   | 1       | x 9…12        |
| a page       | 4 × 2   | x 12…24       |
| a gap        | 1       | y 6…9, 15…18  |

Across: 3 + 1 + 4 = 8. Down: 2 + 1 + 2 + 1 + 2 = 8. Both sums are asserted, so
the mark stays a construction rather than becoming a drawing that once had one.

**The corner radius is zero,** and that is a legibility decision before it is a
taste one. The built-in themes are flat because a projector turns a radius and
a shadow to mud before it loses anything else; a mark that broke the rule would
be the one asset in the repository exempt from the argument the rest of it
makes.

**Why it survives 16 pixels.** The smallest feature is one module, which is an
eighth of the mark, so at the 16-pixel minimum a gap is 2 device pixels — still
a gap rather than a smear. Below 16 the pages stop resolving and the mark reads
as one block, which is why 16 is the stated floor and why it is in the tokens.

**Four rectangles and nothing else.** No path, no circle, no text. There is a
test for that: a mark built from more primitives than it needs is a mark that
does not survive a browser tab.

### The forms it ships in

| file                              | for                                                          |
| --------------------------------- | ------------------------------------------------------------ |
| `mark-light.svg`, `mark-dark.svg` | a page that follows the reader's scheme                      |
| `mark-mono.svg`                   | one colour, taking `currentColor` from whatever holds it     |
| `tile-light.svg`, `tile-dark.svg` | an app icon, inset so a platform's crop has something to cut |

The tile's inset is the brand's clear space — the width of the document bar —
which puts everything drawn inside 80% of the tile and therefore inside every
maskable safe zone in use.

## The wordmark

![The slidx wordmark](../assets/brand/wordmark-light.svg)

**It is set type, not drawn letterforms,** and that is forced rather than
chosen. A drawn wordmark means either a downloaded typeface — which breaks the
one promise the whole repository keeps — or a path nobody can edit and nothing
can check. So `slidx` is the brand's sans stack, which is _the default theme's
stack_, read from it rather than repeated, at weight 650 and −0.02em tracking.
The wordmark and a heading on a slide resolve to the same face on the same
machine.

**Always lowercase.** The crate is `slidx_*`, the command is `slidx`, the
packages are `@slidx/*`. A capitalised wordmark would be a second spelling of
the product's name.

## The lockup

![The slidx lockup](../assets/brand/lockup-light.svg)

One rule per relationship, each a multiple of the mark's own module, so the
lockup scales without a second table of numbers.

| relationship | rule                                                   |
| ------------ | ------------------------------------------------------ |
| size         | the wordmark is set at the mark's height               |
| alignment    | centres, not baselines                                 |
| gap          | 2 modules — a quarter of the mark's height             |
| clear space  | 3 modules on every side, the width of the document bar |
| minimum size | a 17-pixel mark                                        |

**Why centres and not baselines.** The stack is a system stack and resolves to
a different face on every platform, so its cap height is not knowable here. A
rule that claimed one would be precisely wrong on four platforms out of five;
aligning centres is a hair less optically perfect and right everywhere.

**Why 17 pixels.** The wordmark is set at the mark's height, and 17 is the
brand's body size. Below that the wordmark is set smaller than body text, which
is the point at which it stops being a wordmark. The mark _alone_ goes down to
16, because nothing there has to be read as language.

`lockup-light.svg` and `lockup-dark.svg` reserve the clear space inside their
own boxes, so a page that places one flush against something else still gets
it. A rule that only lived in this document would be a rule nobody applies.

There is no stacked lockup. Adding one would mean a second set of alignment
rules and nothing needs one yet.

### What not to do

- **No shadow, no gradient, no glow.** Enforced: `vp run check:flat`.
- **No corner radius,** on the mark or on a container holding it. The token is
  `0` and a test says so.
- **No rotation, no skew, no non-uniform scale.** The construction is the mark;
  a stretched grid is a different mark.
- **No recolouring** beyond the palette below. The pages are signal, the
  document is ink, and the one-colour form is for when only one is available.
- **No text inside the mark.** That is what the lockup is for.
- **No mark below 16 pixels, no lockup below 17.**
- **No mark on a photograph or a pattern.** It needs a flat field; there is a
  tile for the places that crop.
- **No second typeface** for the wordmark, and no outlining it into paths — see
  above for why that is not a stylistic preference.

## Colour

Five roles, and naming them by job rather than by hue is what stops a fourth
blue arriving next year.

| role       | light     | dark      | job                                         |
| ---------- | --------- | --------- | ------------------------------------------- |
| **paper**  | `#fbfbfc` | `#0b0b0d` | what the brand is drawn _on_                |
| **ink**    | `#101014` | `#f2f2f5` | what words are set _in_                     |
| **signal** | `#1b3bc9` | `#b8ccff` | the only colour allowed to _mean_ something |
| muted      | `#4b4b55` | `#a6a6b2` | secondary text                              |
| line       | `#d8d8de` | `#2b2b33` | a hairline                                  |

**Signal carries meaning and nothing else** — a link, an accent rule, the pages
in the mark. Nothing decorative may use it, because a colour used for
decoration cannot also be used for emphasis.

**Neither scheme reaches full black on full white.** At full separation the
edges of a glyph halate, which makes text harder to read rather than easier.
The high-contrast deck theme stops short of it for the same reason and says so
in its own source.

**A dark scheme is not an inversion.** `#1b3bc9` on near-black measures 1.9:1,
so the signal has to move to the other side of the paper it now sits on.

### The signal is a stop deeper than it looks like it should be

This is the part worth reading. The obvious signal for this palette was the
default theme's accent, `#1d4ed8`. On brand paper in a bright room it measures
**4.46:1** — a fail, by four hundredths, on the exact check slidx exists to
run. Nobody would have caught that by looking.

So the palette goes through `slidx_lint` itself: the same `lint()` function,
the same 4.5:1 floor, the same projector-washout model, both schemes, every
lighting condition from a direct panel to a bright room, plus a hall with the
lights up. `slidx_brand::audit` is where that happens, and
`the_audit_is_not_vacuous` re-runs the near-miss blue to prove the audit is
still measuring something.

One honest adjustment: the _legibility_ rule is given a reading distance rather
than a back row. It holds text to the angular size it subtends from the
audience, and a documentation page is read at half a metre — applying fifteen
metres to a 17px paragraph would report every heading on the site as unreadable
while measuring nothing true. That is the same model with the distance that
actually applies, and at reading distance the body floor comes out near 6.5px,
so 17 has real headroom. Both bounds are asserted, including that the rule
still fires on 4px type.

## Type and spacing

The same machinery as a deck theme, because the brand and the themes should be
one system rather than two.

- The scale is `slidx_theme::TypeScale`: **17px base, 1.25 ratio**, code at
  0.94. Same struct, same modular ratio — so there is no arbitrary size to
  reach for here either.
- The spacing is `slidx_theme::Spacing`, every value a multiple of one **8px
  step**: block 3 steps, padding 4 steps, radius 0, hairline 1px.
- The font stacks are read off `slidx_theme::default_theme()` rather than
  repeated, so they cannot drift and a CJK fallback added there arrives here.

**The base differs from a theme's 32px, and only the base.** A slide is read
from row fifteen and a documentation page from fifty centimetres, and
`slidx_lint`'s angular model is precisely the thing that says those are
different numbers.

## Flatness, as a gate

```bash
vp run check:flat
```

`scripts/check-flat.mjs` walks every shipped stylesheet, theme, logo and
generated asset — Rust string literals, TypeScript, CSS, SVG — and exits
non-zero on a `box-shadow`, a `text-shadow`, a `drop-shadow()`, an
`feDropShadow`, any `gradient()`, or an SVG gradient element. It is part of
`ci:conventions`, so the rule cannot be broken quietly, only deliberately and
in review.

Unlike the file-size guideline, it fails rather than warns. The size guideline
is a judgement call about how much design one file is holding; a shadow is not.

Two things it deliberately gets right, both in `scripts/flat.mjs`:

**A mention is not a declaration.** This repository is full of sentences saying
shadows are forbidden — the built-in themes' module docs, the editor
stylesheet's header, the comment inside the mark's own SVG. What is matched is a
property followed by `:` or `=`, a function followed by `(`, an element
followed by a word boundary. A checker that failed on its own documentation
would be switched off inside a week.

**It is not applied to an author's deck.** `examples/` is outside the scan.
slidx does not forbid a shadow in somebody else's slide; the rule is about what
the framework itself ships, and confusing the two would make it opinionated
about content.

The one thing it cannot see is a value assembled at runtime from pieces.
Nothing does that, a checker built to survive deliberate evasion would be a
parser, and the bound is stated rather than papered over.

## What this deliberately did not touch

The four built-in deck themes. A theme is what an author's audience sees, and
restyling one is a different decision from giving the project a brand — an
author who chose `editorial` last year did not ask for it to change. If the
brand suggests a built-in theme, that is a new theme with its own entry in
`builtin.rs` and its own audit, not an edit to an existing one.
