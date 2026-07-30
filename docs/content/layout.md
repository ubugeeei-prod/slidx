---
title: Layout and regions
summary: How a slide is divided, and how one block chooses where it goes.
section: reference
order: 2
---

# Layout and regions

A slide picks a layout, and the layout offers named regions. A block says which
region it belongs in, and everything else lands in the default one in the order
it was written.

```md
---
layout: aside
---

## How the pipeline fits together

{.side}
![The four stages, end to end](./pipeline.svg)
```

That is the whole mechanism: one word in the frontmatter, one line above a
block.

## Why a region and not a rectangle

The obvious way to let someone move something is a freeform canvas, and what it
puts in the file is four floats. Nobody can review them, they mean a different
thing at a different aspect ratio, and no rule can reason about them — a linter
cannot tell you whether text will be legible in a box whose width it only learns
at runtime.

A region is the same gesture with a name. `{.side}` is one word in a diff, it
still means the right thing on a 4:3 projector in a hall that has not been
refitted since 2009, and it is something the overflow rule can measure, because
the geometry belongs to the layout rather than to the slide.

## The layouts

Each region name below is what you write in an attribute line.

<!-- slidx-docs: layouts -->

A layout is one `grid-template-areas` and a region's name _is_ its grid area, so
one string describes the geometry rather than two that can disagree. The slide
is already a size container, so the grid inside it inherits the scaling for
free: every region is a share of the slide, at every projector size, with no
transform and no script.

## The attribute line

An attribute group on a line of its own attaches to the block below it:

```md
{.side}
![The pipeline](./pipeline.svg)
```

It is the same grammar you have already met twice — `{#key .class prop=value}`.
It appears after a span of text as a [mark](steps.md#addressing-part-of-a-line),
after a fence's language to [publish a snippet](tonight.md#somebody-is-going-to-ask-for-the-code),
and here on a line of its own. One grammar, written down once, because a second
parser for it would be a second set of answers about what `prop="two words"`
means.

A line that starts with `{` and does not parse as a group is ordinary content,
and nothing is said about it. Someone writing a paragraph that begins with a
brace is not making a mistake.

## Naming a region the layout does not have

The block lands in the default region, and the build reports it. Both halves
matter: the slide still renders, because a deck edited twenty minutes before a
talk has to render, and the mistake is named, because a block that silently went
somewhere else is exactly the kind of thing you discover from the stage.

The finding comes from the theme rather than from the linter. The regions belong
to the layout, and the linter deliberately knows nothing about themes.

## Overflow is measured per region

A region is its own box. A column a third of the width holds a third of the
line, and a slide's own scroll height never notices that the bottom of one
column has gone.

So every region is measured separately and each gets its own finding, naming the
region. "The slide is too tall" sends you to the wrong half of it; the help says
to move a block rather than to split the slide. See [Lint rules](lint.md) for
what else the overflow group covers, and for why _unchecked_ is not _clean_.

## Dragging a block into a region

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../media/editor-arrange-dark.png">
  <img alt="A paragraph dragged from the left column of a slide into the right one, while the Markdown file beside it gains the line {.right}" src="../media/editor-arrange-light.png">
</picture>

The canvas and the file are two views of one document, and that is the claim this
recording exists to settle. The ghost snaps to the region and the position the
file is about to say — not to the cursor — so what you see under the pointer
mid-drag is what the diff will be. The drop is one operation, so it is one press
of undo, and it writes `{.right}`: one line, above the block it moves.

Everything about the gesture has a key as well. The grips are buttons, so the tab
order walks the blocks of the slide and the arrow keys move one — up and down
through its region, left and right between regions. A deck that could only be
arranged with a pointer would be a deck half the people who write one cannot
arrange at all.

Dragging into the **default** region takes the class away rather than writing
one, because the two say the same thing to the renderer and only one of them is a
line in the diff. So a drag out of a region and back again costs nothing at all.

`vp run record:editor` regenerates the recording by performing the drag against a
real dev server, so a gesture that stopped working fails to reproduce rather than
leaving a picture of something that no longer happens.
