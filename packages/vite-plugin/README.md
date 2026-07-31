# @slidxjs/vite-plugin

Author, preview, and build [slidx](https://github.com/ubugeeei-prod/slidx) decks with Vite.

Making the slides is the short part. The 18px body text nobody could read from
row 12, the colour pair the projector washed out, the fonts on a CDN behind
venue Wi-Fi, the publishing that never happened because you were exhausted —
none of that happens in the editor. This plugin makes it build-time work.

## Start

```bash
npm i -D @slidxjs/vite-plugin
```

```ts
// vite.config.ts
import { defineConfig } from "vite";
import { slidx } from "@slidxjs/vite-plugin";

export default defineConfig({ plugins: [slidx()] });
```

That is the whole configuration.

```bash
vite dev # the deck, plus the visual editor at /__slidx/
vite build # one HTML document per slide
```

Slides are Markdown files in `slides/`, one per file. A build writes a page per
slide, a presenter view, a print shell, an OG image per slide, a PDF, and a
sitemap.

## The editor is over the same file

`/__slidx/` is served in development and nowhere else. Its canvas is the deck's
own page rather than a preview of it, and every change is a byte-range splice
into the file you saved — so your blank lines, your `*` bullets and your
hand-wrapped paragraph all survive, and one drag is one line in the diff.

Two people can edit at once. The dev server holds the one document, the roster
says who is on which slide, and a mark on the canvas says which paragraph they
are in.

## What it checks before you leave your desk

Contrast under a projector's washout, rendered type against angular size from
the back row, content that overflows measured in a real browser, image
resolution against the size it is drawn at, a step that addresses nothing, and
a per-slide time budget summed against the length of your slot.

A remote asset is an error rather than advice: a built deck asks nothing of any
origin but its own, and that is asserted in Chromium, Firefox and WebKit.

## Documentation

https://github.com/ubugeeei-prod/slidx#readme

## License

MIT. The notice is in this package, and at
https://github.com/ubugeeei-prod/slidx/blob/main/LICENSE.
