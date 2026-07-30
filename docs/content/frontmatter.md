---
title: Frontmatter
summary: Every key slidx reads, where it means anything, and what it accepts.
section: reference
order: 1
---

# Frontmatter

A deck is Markdown files separated by `---`, and each slide may open with a
YAML block. Every table on this page is rendered from
[`slidx_lsp::vocabulary`](../../crates/slidx_lsp/src/vocabulary.rs), the module
the language server reads to answer completion — so the site and your editor
show you the same words, because there is only one set of them.

Frontmatter is deliberately open. A key slidx has never heard of is kept rather
than rejected, so a theme or a plugin can read one, and so an author who writes
a key for their own tooling does not have to fight the parser.

## Where a key means something

The first block in a deck is doing two jobs: it configures the deck, and it
configures the first slide. Every later block configures one slide only. That is
why the keys are split into two tables rather than one — `title:` in a later
block silently does nothing, and there is no way to make that visible except to
say which is which.

### Deck keys, in the first block only

<!-- slidx-docs: frontmatter-deck -->

### Slide keys, in any block

<!-- slidx-docs: frontmatter-slide -->

## What the closed sets contain

Several keys accept one of a fixed set of values. Each list below is derived
from the definition the compiler already checks against, so a value here is a
value slidx accepts.

`layout:` is one of them, and it has [a page of its own](layout.md) because the
value is only half of it — the other half is which region each block chooses.

### `theme:`

Four built in, each holding both a light and a dark variant — the room's
lighting is usually unknown until the day. Every colour in them is a token, and
the built-in themes are held to the linter's own contrast rules, so a theme
cannot ship something illegible.

<!-- slidx-docs: themes -->

### `aspect:`

<!-- slidx-docs: aspects -->

### `transition:`

A slide that names a transition decides for itself, including when it says
`none`; only silence inherits the deck's. `transition: false` is accepted as a
spelling of `none`, because YAML reads it as a boolean and a slide switching a
deck-wide transition off would otherwise read as a slide that said nothing.

<!-- slidx-docs: transitions -->

### `autoSteps:`

Staging without writing a step list. See [Steps](steps.md) for what a stop is
and what else can produce one.

<!-- slidx-docs: auto-steps -->

## A slide, with most of it

```md
---
title: Making Decks Fast
event: SlidxConf 2026
duration: 20m
theme: editorial
aspect: "16:9"
---

# Making Decks Fast

<!-- notes:
Open with the outcome, not the agenda.
-->
```

Notes live in an HTML comment beginning `notes:`, which keeps them out of the
rendered slide and in the same file as the words they are about.

## A key slidx does not read

Nothing happens, and nothing complains. That is the open half of the design.
If you expected a key to do something and it did not, check the spelling against
the tables above rather than the parser — both `camelCase` and `kebab-case` are
accepted for every key, so `autoSteps` and `auto-steps` are the same key and a
typo in the middle is not.
