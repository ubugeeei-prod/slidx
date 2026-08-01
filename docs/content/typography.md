---
title: How type is set
summary: Leading, tracking and measure, where they come from, and what changes when a deck is written in Japanese.
section: reference
order: 3
---

# How type is set

A theme picks a base size and a ratio, and every size on a slide comes from
those. This page is about the three things a size does not decide — how far
apart the lines sit, how tightly the letters do, and how long a line may run —
and about what a deck written in Japanese gets that a deck written in English
does not.

None of it is configured on a slide. It is emitted as custom properties by the
theme, so a block that overrides one is overriding a value with a reason behind
it.

## The two curves

Both quantities move with size, in opposite directions.

**Tracking closes as type grows.** Sidebearings scale with the glyph and the
eye's tolerance for the gap between two letters does not, so a face set at
display size looks loose at the tracking that suits it at text size. Tracking is
therefore zero at the theme's own base and moves from there: negative above,
**positive below**. A caption wants more tracking than body text, which is the
half a single hard-coded value cannot express.

**Leading closes as type grows,** for the same reason and one more — leading is
set against the line beneath it, and a heading is short.

Both are anchored at the theme's base, so a theme states one leading and gets a
coherent ladder. Two themes with different scales get different numbers, which
is the point:

|               | `minimal` (32px, 1.25) | `editorial` (34px, 1.333) |
| ------------- | ---------------------- | ------------------------- |
| body leading  | 1.50                   | 1.60                      |
| `h1` leading  | 1.15                   | 1.15                      |
| `h1` tracking | −0.0147em              | −0.0190em                 |

`editorial` sets its body more open because it is the prose theme. Its `h1` is
_tracked_ tighter — a 1.333 ratio puts that heading much further from the base
than a 1.25 ratio does, and the curve closes with the distance. The two `h1`
leadings landing on the same number is the two effects cancelling: a higher
anchor, a longer way down.

## Measure

A line is too long when the eye loses its place returning to the next one. The
readable range is conventionally quoted in characters — 45–75 for Latin prose,
fewer for display type — and slidx quotes it as one length in `em` instead:

|                           |        |
| ------------------------- | ------ |
| `--slidx-measure-prose`   | `30em` |
| `--slidx-measure-heading` | `13em` |

One number for two scripts, because the two ratios cancel. A Han glyph or a kana
occupies a full em where a Latin lowercase averages about half of one — and it
also carries about twice as much, since Japanese renders into roughly twice the
character count in English. Thirty em is sixty Latin characters or thirty
Japanese ones, and those are the same sentence.

`em` resolves against the element's own size, so one declaration is the right
measure at every step of the scale. This used to be `22ch` — the advance of `0`,
a Latin metric — which came out at about twelve characters of Japanese and broke
a heading mid-word.

It is a **cap and never a target**: a block still fits its content, so a
three-word line stays three words wide.

## Japanese, Chinese and Korean

Everything above is script-neutral. Four things are not, and a deck gets them
when its document language is `ja`, `zh` or `ko`.

**Leading opens by 0.2.** Latin line separation is partly done by the whitespace
above the x-height and below the baseline, which is why 1.5 looks generous
there. A CJK line is a run of filled em boxes with no such margin, so the same
ratio sets visibly tighter. Body text lands at 1.7, inside the 1.5–2.0 range
Japanese practice quotes for 行送り.

**Tracking is zero, and `palt` does its job instead.** A kanji is drawn to fill
its em box with almost no sidebearing, so tracking a Latin face has room to give
up would come out of the strokes. `font-feature-settings: "palt"` asks the font
for the proportional advances it already contains — on headings, which is where
the convention puts it.

**約物 are trimmed.** A Japanese bracket or full stop is drawn inside a full em
box with the ink on one side, so `「約物」の` sets with holes either side of the
quotes. `text-spacing-trim` is the typographic answer, and it is the one thing
here a reader notices immediately without being able to say why.

**Lines break at a 文節 boundary.** Japanese has no spaces, so the default
breaks wherever the line runs out of room:

```text
  before                          after
  ・行末の禁則処理が甘く、        ・行末の禁則処理が甘く、句読点が行頭に
    句読点が行頭に来てしまうことがあ    来てしまうことがある。
    る。
```

`word-break: auto-phrase` is applied to prose as well as to headings, which is a
choice. The usual advice reserves it for display type, because a phrase boundary
leaves a more ragged right edge. On a slide every line is display type: three
lines long, read from row fifteen, with no column of text for the raggedness to
spoil.

`line-break: strict` is on for the same reason — the default lets a line begin
with a small kana or a 長音符, and a slide is the place that shows.

### Which browsers do which

Measured in the three engines the matrix covers, rather than copied from a
support site. `packages/vite-plugin/test/browser.test.ts` asks each of them and
fails if the answer here is wrong — including when a browser _gains_ one, so the
degradation below stops being described the moment it stops happening.

|                            | Chromium | Firefox | WebKit |
| -------------------------- | -------- | ------- | ------ |
| leading, tracking, measure | ✓        | ✓       | ✓      |
| `line-break: strict`       | ✓        | ✓       | ✓      |
| `palt`                     | ✓        | ✓       | ✓      |
| `text-spacing-trim`        | ✓        | —       | —      |
| `word-break: auto-phrase`  | ✓        | —       | —      |

A deck that gets neither of the last two is set the way a browser sets Japanese
on its own — which is where every slide starts. Everything above them still
applies, so it is a degradation rather than a break, and it is the same bargain
the cross-document transitions make.

## Which language a deck is in

`lang:` in frontmatter, whenever a deck declares one:

```md
---
lang: ja
---
```

A deck that declares nothing used to be served as `lang="en"` — a specific,
confident answer that was wrong for every deck not written in English, and one
that three things believed: a screen reader picked an English voice, the browser
applied Latin line-breaking, and none of the setting above applied.

So the slides are now read when frontmatter is silent. Kana proves Japanese and
hangul proves Korean, since neither is used in another language; Han with
neither is Chinese, because Japanese prose cannot avoid kana — particles and
okurigana are kana. It takes a **majority** of the letters rather than a
presence test, so an English talk quoting one Japanese phrase stays English.

A declared `lang:` always wins, and the detection never returns a Latin tag:
telling English from German needs a different kind of evidence, and `en` is
already the default.

## Overriding a value

Every number above is a custom property, so a slide can change one through the
tagged style block the editor already writes into:

```md
<style data-slidx>
:root {
  --slidx-measure-prose: 44em;
}
</style>

A slide whose prose wants a longer line than the theme gives it.
```

Only `--slidx-*` declarations inside a `data-slidx` block belong to this model,
and the renderer scopes them to that slide's own element rather than to the
document — which is what makes it correct in the print shell, where every slide
shares one page.

There is no frontmatter key for any of it, deliberately. A theme is where a
decision about how type is set belongs; a slide that carried its own leading
would be a slide that stops matching the rest of the deck the moment the theme
changes.
