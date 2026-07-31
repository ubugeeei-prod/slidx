# @slidxjs/theme-workshop

A slidx theme for the room the four built-in themes deliberately do not cover:
a hands-on session, where the audience is typing along and the slide is a
reference they glance up at rather than a thing they watch.

```bash
vp add -D @slidxjs/theme-workshop
```

```md
---
theme: workshop
---
```

That is the whole installation. `@slidxjs/vite-plugin` reads the theme documents
of the packages your project depends on, so nothing has to be imported,
registered, or named in `vite.config.ts`.

## What it decides

|                                   |                                                                      |
| --------------------------------- | -------------------------------------------------------------------- |
| Code at body size                 | the line being copied is the content; the prose is the caption on it |
| The tightest padding of any theme | every pixel of it is a character that wraps                          |
| A narrow type scale               | a heading should not take the room a snippet needs                   |
| Light first                       | a workshop room stays lit so people can see their keyboards          |
| A 120ms transition                | a workshop deck is stepped backwards as often as forwards            |

## What it is

One JSON file of tokens — a palette, a type scale, spacing and motion — and a
`slidx.theme` key in `package.json` pointing at it. No CSS, no stylesheet, no
code. That is what lets slidx's linter hold this theme to the same contrast and
legibility rules it holds a deck to, in every room it models, before anyone
stands in front of it.

Nothing here reaches for a webfont, and nothing here can: a built deck makes
zero network requests, and a font stack naming a remote face is refused at load
rather than discovered at the venue.

## License

MIT
