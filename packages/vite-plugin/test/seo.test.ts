/**
 * What a crawler and a link preview actually receive.
 *
 * Composing a canonical link is not the feature. *Emitting* one is, and the
 * difference is the whole of this file: every assertion here reads a file that
 * a real `vite build` wrote, with `plugins: [slidx()]` and no options, because
 * that is the configuration the front page promises and the only one most decks
 * will ever have.
 *
 * This repository has a section of its roadmap about the alternative. Three
 * features were implemented, tested, merged, and unreachable — the code that
 * would have done the work existed and nothing called it. A `metaTags` function
 * sat in `og.ts` for the same length of time, pointing at cards no page ever
 * mentioned, and no unit test noticed because a unit test asks the function.
 */

import { mkdtemp, mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { build } from "vite";
import { beforeAll, describe, expect, it } from "vite-plus/test";

import { slidx } from "../src/index";

/** Where the deck under test claims to be deployed. */
const ORIGIN = "https://example.com/talk/";

const PUBLISHED = `---
title: Making Decks Fast
draft: false
url: ${ORIGIN}
author: ubugeeei
event: SlidxConf 2026
date: 2026-05-14
---

# Making Decks Fast

A framework for the whole life of a talk.
`;

/** The same deck with nothing said about publishing it, which is the default. */
const UNDECLARED = PUBLISHED.replace("draft: false\n", "");

const SECOND = `# What goes wrong

Almost everything that does happens outside the editor.
`;

const THIRD = `# Thanks

<!-- notes: The rewrite paid for itself in a fortnight. -->
`;

interface Built {
  root: string;
  files: string[];
  read: (file: string) => Promise<string>;
}

async function buildDeck(
  slides: Record<string, string>,
  options: Parameters<typeof slidx>[0] = {},
): Promise<Built> {
  const root = await mkdtemp(join(tmpdir(), "slidx-seo-"));
  await mkdir(join(root, "slides"), { recursive: true });

  for (const [name, source] of Object.entries(slides)) {
    await writeFile(join(root, "slides", name), source);
  }

  await build({
    root,
    logLevel: "silent",
    plugins: [slidx(options)],
    build: { outDir: join(root, "dist") },
  });

  return {
    root,
    files: await walk(join(root, "dist")),
    read: (file: string) => readFile(join(root, "dist", file), "utf8"),
  };
}

async function walk(directory: string, prefix = ""): Promise<string[]> {
  let entries;
  try {
    entries = await readdir(directory, { withFileTypes: true });
  } catch {
    return [];
  }

  const found: string[] = [];

  for (const entry of entries) {
    const path = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) {
      found.push(...(await walk(join(directory, entry.name), path)));
    } else {
      found.push(path);
    }
  }

  return found.sort();
}

/** The `content` or `href` of the first tag matching a pattern. */
function valueOf(html: string, pattern: RegExp): string | null {
  return pattern.exec(html)?.[1] ?? null;
}

const CANONICAL = /<link rel="canonical" href="([^"]*)">/;
const DESCRIPTION = /<meta name="description" content="([^"]*)">/;
const OG_IMAGE = /<meta property="og:image" content="([^"]*)">/;

/** Every `<loc>` in a sitemap, in document order. */
function locations(xml: string): string[] {
  return [...xml.matchAll(/<loc>([^<]*)<\/loc>/g)].map((match) => match[1] ?? "");
}

describe("a published deck, built with no configuration at all", () => {
  let built: Built;

  beforeAll(async () => {
    built = await buildDeck({ "0001.md": PUBLISHED, "0002.md": SECOND, "0003.md": THIRD });
  }, 60_000);

  it("gives every page a canonical URL that points at that page", async () => {
    // Two spellings of one convention meet here: Rust works out the URL of
    // slide three and TypeScript works out the file it is written to. A test
    // that only read the canonical would pass while they disagreed.
    for (const [file, expected] of [
      ["slides/index.html", ORIGIN],
      ["slides/2/index.html", `${ORIGIN}2/`],
      ["slides/3/index.html", `${ORIGIN}3/`],
    ] as const) {
      expect(valueOf(await built.read(file), CANONICAL), file).toBe(expected);
    }
  });

  it("lists exactly the audience pages in the sitemap, and nothing else", async () => {
    // Not the presenter view, which carries the speaker's notes. Not the print
    // shell, which is every slide again and would outrank the slides. Not the
    // cards, which are images.
    expect(locations(await built.read("slides/sitemap.xml"))).toEqual([
      ORIGIN,
      `${ORIGIN}2/`,
      `${ORIGIN}3/`,
    ]);
  });

  it("writes a robots.txt at the site root that points back at the sitemap", async () => {
    const robots = await built.read("robots.txt");

    expect(built.files).toContain("robots.txt");
    expect(robots).toContain(`Sitemap: ${ORIGIN}sitemap.xml`);
    // The file it names has to be the file that was written.
    expect(built.files).toContain("slides/sitemap.xml");
  });

  it("keeps a crawler away from the pages that carry the notes", async () => {
    const robots = await built.read("robots.txt");
    const presenter = await built.read("slides/2/presenter/index.html");

    expect(robots).toContain("Disallow: /slides/presenter/");
    expect(robots).toContain("Disallow: /slides/*/presenter/");
    expect(presenter).toContain('<meta name="robots" content="noindex">');
    expect(await built.read("slides/print/index.html")).toContain('content="noindex"');
  });

  it("describes each slide in its own words rather than the deck's", async () => {
    // Forty pages sharing one description is forty search results nobody can
    // tell apart, thirty-nine of which describe another page.
    const descriptions = await Promise.all(
      ["slides/index.html", "slides/2/index.html", "slides/3/index.html"].map(async (file) =>
        valueOf(await built.read(file), DESCRIPTION),
      ),
    );

    expect(descriptions).toEqual([
      "A framework for the whole life of a talk.",
      "Almost everything that does happens outside the editor.",
      // A slide with nothing but a heading falls back to what the author said
      // about it, which is the speaker notes.
      "The rewrite paid for itself in a fortnight.",
    ]);
  });

  it("points each page at the card that was drawn for that slide", async () => {
    // The cards have been emitted since M1 and nothing in any document
    // mentioned them, so every link to a deck was a grey rectangle.
    expect(valueOf(await built.read("slides/2/index.html"), OG_IMAGE)).toBe(`${ORIGIN}og-2.png`);
    expect(built.files).toContain("slides/og-2.svg");
  });

  it("carries structured data a machine can parse out of the page", async () => {
    const html = await built.read("slides/2/index.html");
    const block = /<script type="application\/ld\+json">(.*?)<\/script>/s.exec(html)?.[1];

    const data = JSON.parse(block ?? "{}");

    expect(data["@type"]).toBe("PresentationDigitalDocument");
    expect(data.url).toBe(`${ORIGIN}2/`);
    expect(data.isPartOf.name).toBe("Making Decks Fast");
    expect(data.recordedAt.name).toBe("SlidxConf 2026");
    expect(data.author.name).toBe("ubugeeei");
  });

  it("links the slides to each other as the sequence they are", async () => {
    const middle = await built.read("slides/2/index.html");

    expect(middle).toContain('<link rel="prev" href="../">');
    expect(middle).toContain('<link rel="next" href="../3/">');
    expect(await built.read("slides/index.html")).not.toContain('rel="prev"');
    expect(await built.read("slides/3/index.html")).not.toContain('rel="next"');
  });

  it("says nothing about not being indexed", async () => {
    expect(await built.read("slides/index.html")).not.toContain("noindex");
  });
});

/**
 * The judgement the issue turns on.
 *
 * A deck sits in a repository for weeks before a conference announces the talk,
 * and a search engine that has crawled an embargoed deck cannot be told to
 * forget it on the speaker's schedule. So the safe answer is the default, and
 * the author states the other one.
 */
describe("a deck that has not said it is public", () => {
  let built: Built;

  beforeAll(async () => {
    built = await buildDeck({ "0001.md": UNDECLARED, "0002.md": SECOND });
  }, 60_000);

  it("asks every page not to be indexed", async () => {
    for (const file of ["slides/index.html", "slides/2/index.html"]) {
      expect(await built.read(file), file).toContain('<meta name="robots" content="noindex">');
    }
  });

  it("offers no slide in its sitemap", async () => {
    // The file still exists, because a deck that was public last week is
    // already in a crawler's queue and an empty list retracts it.
    const xml = await built.read("slides/sitemap.xml");

    expect(locations(xml)).toEqual([]);
    expect(xml).toContain("<urlset");
  });

  it("disallows the whole deck in robots.txt, and names no sitemap", async () => {
    const robots = await built.read("robots.txt");

    expect(robots).toContain("Disallow: /slides/");
    expect(robots).not.toContain("Sitemap:");
  });

  it("still says which URL is the real one", async () => {
    // Not indexed is not the same as not deployed. The deck is at one address
    // whether or not anybody is invited to it.
    expect(valueOf(await built.read("slides/index.html"), CANONICAL)).toBe(ORIGIN);
  });
});

describe("a deck nobody has given an address", () => {
  let built: Built;

  beforeAll(async () => {
    built = await buildDeck({ "0001.md": "---\ndraft: false\n---\n\n# One\n", "0002.md": SECOND });
  }, 60_000);

  it("claims no canonical rather than guessing one", async () => {
    // A guessed origin is not a smaller version of the right answer: it tells a
    // search engine to prefer a page that does not exist.
    const html = await built.read("slides/index.html");

    expect(html).not.toContain("canonical");
    expect(html).not.toContain('href="http');
  });

  it("writes no sitemap, because a sitemap of relative URLs is an invalid file", () => {
    expect(built.files).not.toContain("slides/sitemap.xml");
  });

  it("still writes a robots.txt, because none of it needed an origin", async () => {
    expect(await built.read("robots.txt")).toContain("User-agent: *");
  });

  it("still points at its own cards and its own neighbours", async () => {
    const second = await built.read("slides/2/index.html");

    expect(valueOf(second, OG_IMAGE)).toBe("../og-2.png");
    expect(second).toContain('<link rel="prev" href="../">');
  });
});

describe("a deployment that knows its address better than the deck does", () => {
  it("takes the URL from the option over the one in the file", async () => {
    // A preview build of the same deck is at a different origin, and editing
    // the deck per environment is how the two get out of step.
    const built = await buildDeck(
      { "0001.md": PUBLISHED },
      { deckUrl: "https://preview.example.com/pr-12/" },
    );

    expect(valueOf(await built.read("slides/index.html"), CANONICAL)).toBe(
      "https://preview.example.com/pr-12/",
    );
    expect(locations(await built.read("slides/sitemap.xml"))).toEqual([
      "https://preview.example.com/pr-12/",
    ]);
  }, 60_000);
});

describe("a project that already has a robots.txt", () => {
  it("leaves it alone, and keeps the deck out of the index the other way", async () => {
    // That file belongs to the whole site. Replacing it could open a site up as
    // easily as close one down, so the `noindex` on every page is what holds —
    // which is why a draft deck says it both ways rather than either.
    const root = await mkdtemp(join(tmpdir(), "slidx-seo-"));
    await mkdir(join(root, "slides"), { recursive: true });
    await mkdir(join(root, "public"), { recursive: true });
    await writeFile(join(root, "slides", "0001.md"), UNDECLARED);
    await writeFile(join(root, "public", "robots.txt"), "User-agent: *\nDisallow: /admin/\n");

    await build({
      root,
      logLevel: "silent",
      plugins: [slidx()],
      build: { outDir: join(root, "dist") },
    });

    const robots = await readFile(join(root, "dist", "robots.txt"), "utf8");
    const slide = await readFile(join(root, "dist", "slides", "index.html"), "utf8");

    expect(robots).toBe("User-agent: *\nDisallow: /admin/\n");
    expect(slide).toContain('<meta name="robots" content="noindex">');
  }, 60_000);
});

describe("a deck served from the site root", () => {
  it("names the root in robots.txt when it is a draft", async () => {
    const built = await buildDeck({ "0001.md": UNDECLARED }, { base: "/" });
    const robots = await built.read("robots.txt");

    expect(robots).toContain("Disallow: /\n");
    expect(built.files).toContain("sitemap.xml");
  }, 60_000);
});
