/**
 * The install-to-build path, end to end.
 *
 * slidx claims two commands take someone from nothing to a deployable deck.
 * That claim is only worth making if it is checked, and it cannot be checked
 * by unit tests: it depends on Vite's hook order, on rollup accepting a
 * virtual entry, and on the wasm module loading in the build process. So this
 * runs a real build against a real directory of Markdown.
 */

import { mkdtemp, mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { build } from "vite";
import { beforeAll, describe, expect, it } from "vite-plus/test";

import { EDITOR_ROUTE_PREFIX, slidx } from "../src/index";

async function buildDeck(
  slides: Record<string, string>,
  options: Parameters<typeof slidx>[0] = {},
): Promise<{ root: string; files: string[] }> {
  const root = await mkdtemp(join(tmpdir(), "slidx-"));
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

  return { root, files: await walk(join(root, "dist")) };
}

async function walk(directory: string, prefix = ""): Promise<string[]> {
  // A build that emits nothing leaves no output directory at all, which is
  // the same answer as an empty one.
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

describe("building a deck with no configuration", () => {
  let result: Awaited<ReturnType<typeof buildDeck>>;

  beforeAll(async () => {
    result = await buildDeck({
      "0001.md": "---\ntitle: Demo\n---\n\n# One\n",
      "0002.md": "## Two\n\n- a point\n",
    });
  }, 60_000);

  it("emits one page per slide", () => {
    expect(result.files).toContain("slides/index.html");
    expect(result.files).toContain("slides/2/index.html");
  });

  it("emits a presenter view for each slide", () => {
    // On by default: the presenter view is the reason to use a deck tool
    // rather than a PDF, and discovering it exists after the talk is too late.
    expect(result.files).toContain("slides/presenter/index.html");
    expect(result.files).toContain("slides/2/presenter/index.html");
  });

  it("ships JavaScript only to the presenter, never to the audience", async () => {
    // The property that makes a deck load instantly on venue Wi-Fi. A stray
    // empty chunk from the virtual entry would break it silently.
    expect(result.files.filter((file) => file.endsWith(".js"))).toEqual(["slides/runtime.js"]);

    const slide = await readFile(join(result.root, "dist/slides/index.html"), "utf8");
    expect(slide).not.toContain("<script");
  });

  it("ships no way to edit the deck it was built from", async () => {
    // The editing routes write to the author's slide files. They are
    // registered in `configureServer` and nowhere else, so `vite build` never
    // creates them — this is the assertion that keeps a deck on a web server
    // from ever offering one.
    const pages = await Promise.all(
      result.files
        .filter((file) => file.endsWith(".html") || file.endsWith(".js"))
        .map((file) => readFile(join(result.root, "dist", file), "utf8")),
    );

    expect(pages.filter((page) => page.includes(EDITOR_ROUTE_PREFIX))).toEqual([]);
  });

  it("emits one printable document for the whole deck", () => {
    // One document rather than one per slide: a handout is printed once, and
    // a browser prints one document at a time.
    expect(result.files).toContain("slides/print/index.html");
  });

  it("draws a social card for every slide, and one for the deck", () => {
    // A deck is shared as a URL far more often than it is presented, and a
    // URL with no card is a grey rectangle in a timeline.
    expect(result.files).toContain("slides/og-1.svg");
    expect(result.files).toContain("slides/og-2.svg");
    expect(result.files).toContain("slides/og.svg");
  });

  it("converts the cards to PNG where a browser exists", () => {
    // Almost no platform renders SVG, so the PNG is the one that matters.
    // Where no browser is installed the SVG stands alone rather than the
    // build failing, so this only asserts the pair when one was produced.
    const png = result.files.filter((file) => file.endsWith(".png"));
    if (png.length > 0) expect(png).toContain("slides/og.png");
  });

  it("emits nothing else", () => {
    expect(result.files.filter((file) => !file.startsWith("slides/og"))).toEqual([
      "slides/2/index.html",
      "slides/2/presenter/index.html",
      "slides/index.html",
      "slides/presenter/index.html",
      "slides/print/index.html",
      "slides/runtime.js",
    ]);
  });

  it("shares one runtime between every presenter page", async () => {
    const presenter = await readFile(
      join(result.root, "dist/slides/2/presenter/index.html"),
      "utf8",
    );
    expect(presenter).toContain('from "/slides/runtime.js"');
  });

  it("writes complete, self-contained documents", async () => {
    const html = await readFile(join(result.root, "dist/slides/index.html"), "utf8");

    expect(html).toMatch(/^<!doctype html>/);
    expect(html).toContain("--slidx-color-text:");
    expect(html).not.toContain("<link");
  });

  it("puts the deck title on every page", async () => {
    const second = await readFile(join(result.root, "dist/slides/2/index.html"), "utf8");
    expect(second).toContain("Demo");
  });
});

describe("options", () => {
  it("serves the deck at the site root when asked", async () => {
    const { files } = await buildDeck({ "0001.md": "# One\n" }, { base: "/" });
    expect(files.filter((file) => !file.startsWith("og"))).toEqual([
      "index.html",
      "presenter/index.html",
      "print/index.html",
      "runtime.js",
    ]);
  }, 60_000);

  it("builds only the audience pages when both extra views are off", async () => {
    const { files } = await buildDeck(
      { "0001.md": "# One\n" },
      { presenter: false, print: false, og: false },
    );
    expect(files).toEqual(["slides/index.html"]);
  }, 60_000);

  it("keeps the print shell without the presenter view", async () => {
    // A speaker who only wants a PDF should not pay for pages they will not
    // open.
    const { files } = await buildDeck({ "0001.md": "# One\n" }, { presenter: false });

    expect(files).toContain("slides/print/index.html");
    expect(files).not.toContain("slides/presenter/index.html");
  }, 60_000);

  it("reads a directory other than ./slides", async () => {
    const root = await mkdtemp(join(tmpdir(), "slidx-"));
    await mkdir(join(root, "talk"), { recursive: true });
    await writeFile(join(root, "talk", "0001.md"), "# One\n");

    await build({
      root,
      logLevel: "silent",
      plugins: [slidx({ srcDir: "talk" })],
      build: { outDir: join(root, "dist") },
    });

    expect(await walk(join(root, "dist"))).toContain("slides/index.html");
  }, 60_000);
});

describe("a deck with nothing in it", () => {
  it("builds rather than crashing", async () => {
    // Every new project starts here. A crash would read as the plugin being
    // broken rather than as there being no slides yet.
    const { files } = await buildDeck({});
    expect(files).toEqual([]);
  }, 60_000);
});
