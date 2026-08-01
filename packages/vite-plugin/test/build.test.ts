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

import { build, createLogger } from "vite";
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

  it("ships JavaScript beside the presenter, never in the audience page", async () => {
    // The property that makes a deck load instantly on venue Wi-Fi. A stray
    // empty chunk from the virtual entry would break it silently.
    expect(result.files.filter((file) => file.endsWith(".js"))).toEqual([
      "slides/rehearsal.js",
      "slides/runtime.js",
    ]);

    // The `<script>` elements on an audience slide are enumerated rather than
    // counted at zero, because neither of them is fetched. One is the
    // structured data, a block of JSON in the container the JSON-LD
    // specification chose for it, which no browser executes. The other is the
    // navigator: a few hundred inline bytes that turn a clicker's keys into a
    // click on a link already in the footer, and the only reason a slide with
    // no steps can be left at all — see `slidx_render::navigation`.
    const slide = await readFile(join(result.root, "dist/slides/index.html"), "utf8");
    expect(slide).not.toContain('<script type="module"');
    expect(slide).not.toMatch(/<script[^>]*\ssrc=/);
    expect(slide.match(/<script/g)).toEqual(["<script", "<script"]);
    expect(slide).toContain('<script type="application/ld+json">');
    expect(slide).toContain(".slidx-slide-nav");
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
    // `robots.txt` and no `sitemap.xml`: this deck has said nothing about where
    // it is deployed, and a sitemap's `<loc>` is a full URL. Every directive in
    // a robots file is root-relative, so that one needs no origin.
    expect(result.files.filter((file) => !file.startsWith("slides/og"))).toEqual([
      "robots.txt",
      "slides/2/index.html",
      "slides/2/presenter/index.html",
      "slides/index.html",
      "slides/presenter/index.html",
      "slides/print/index.html",
      "slides/rehearsal.js",
      "slides/runtime.js",
    ]);
  });

  it("shares one runtime between every presenter page", async () => {
    const presenter = await readFile(
      join(result.root, "dist/slides/2/presenter/index.html"),
      "utf8",
    );
    expect(presenter).toContain('from "/slides/runtime.js"');
    expect(presenter).toContain('from "/slides/rehearsal.js"');
  });

  it("keeps rehearsal out of the shared runtime", async () => {
    const rehearsal = await readFile(join(result.root, "dist/slides/rehearsal.js"), "utf8");
    const runtime = await readFile(join(result.root, "dist/slides/runtime.js"), "utf8");

    expect(rehearsal).toContain("openRehearsalSession");
    expect(rehearsal).not.toMatch(/from\s+["']@slidxjs\//);
    expect(runtime).not.toContain("openRehearsalSession");
  });

  it("writes complete, self-contained documents", async () => {
    const html = await readFile(join(result.root, "dist/slides/index.html"), "utf8");

    expect(html).toMatch(/^<!doctype html>/);
    expect(html).toContain("--slidx-color-text:");
    // Nothing the browser has to go and get. The `<link>` elements a page does
    // carry are `prev` and `next`, which name the neighbouring slides and are
    // not fetched by anything.
    expect(html).not.toContain('<link rel="stylesheet"');
    expect(html).not.toContain('<link rel="preload"');
  });

  it("puts the deck title on every page", async () => {
    const second = await readFile(join(result.root, "dist/slides/2/index.html"), "utf8");
    expect(second).toContain("Demo");
  });
});

describe("a staged deck", () => {
  it("ships the effect stylesheet beside the runtime that loads it", async () => {
    const { root, files } = await buildDeck({
      "0001.md": "# One\n\n- now\n- later <!-- step -->\n",
    });

    expect(files).toContain("slides/effects.css");
    expect(await readFile(join(root, "dist/slides/effects.css"), "utf8")).toContain(
      "[data-slidx-hidden]",
    );
  }, 60_000);
});

describe("options", () => {
  it("serves the deck at the site root when asked", async () => {
    const { files } = await buildDeck({ "0001.md": "# One\n" }, { base: "/" });
    expect(files.filter((file) => !file.startsWith("og"))).toEqual([
      "index.html",
      "presenter/index.html",
      "print/index.html",
      "rehearsal.js",
      // Still the site root, because that is the only place a crawler looks.
      "robots.txt",
      "runtime.js",
    ]);
  }, 60_000);

  it("builds only the audience pages when both extra views are off", async () => {
    const { files } = await buildDeck(
      { "0001.md": "# One\n" },
      { presenter: false, print: false, og: false },
    );
    expect(files).toEqual(["robots.txt", "slides/index.html"]);
  }, 60_000);

  it("keeps the print shell without the presenter view", async () => {
    // A speaker who only wants a PDF should not pay for pages they will not
    // open.
    const { files } = await buildDeck({ "0001.md": "# One\n" }, { presenter: false });

    expect(files).toContain("slides/print/index.html");
    expect(files).not.toContain("slides/presenter/index.html");
    expect(files).not.toContain("slides/rehearsal.js");
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

/**
 * The pages behind the QR codes on a slide.
 *
 * A shared fence draws a code on the slide *and* needs a page for that code to
 * point at. The renderer composed the page from the day the feature landed and
 * nothing wrote it, so every code in a built deck resolved to a 404 — the one
 * failure mode where the audience finds out rather than the author, and finds
 * out by pointing a phone at a wall.
 */
describe("shared code snippets", () => {
  it("writes a page for every shared fence", async () => {
    const { files } = await buildDeck({
      "0001.md": "# Retry\n\n```rust {#retry-policy .share}\nfn retry() {}\n```\n",
    });

    expect(files).toContain("slides/snippets/retry-policy.html");
  }, 60_000);

  it("writes nothing for a deck that shares nothing", async () => {
    // Which is most decks. A snippets directory in every build would be a
    // directory people wonder about.
    const { files } = await buildDeck({ "0001.md": "# One\n\n```rust\nfn main() {}\n```\n" });

    expect(files.filter((file) => file.includes("snippets/"))).toEqual([]);
  }, 60_000);

  it("puts the page where the slide's code says it is", async () => {
    // The QR is drawn from the same path. A page written somewhere else is a
    // code that scans to a 404, which nobody discovers until a room does.
    const { root, files } = await buildDeck({
      "0001.md": "# Retry\n\n```rust {#retry-policy .share}\nfn retry() {}\n```\n",
    });

    const slide = await readFile(join(root, "dist/slides/index.html"), "utf8");
    const written = files.filter((file) => file.includes("snippets/"));

    expect(written).toHaveLength(1);
    expect(slide).toContain("snippets/retry-policy.html");
  }, 60_000);
});

/**
 * The image rules, reaching a build.
 *
 * They need a file's own pixel dimensions and they run inside WebAssembly,
 * which has no filesystem — so for as long as nothing read the headers on this
 * side, the rules were implemented, tested, merged, and never once ran on a
 * real deck. The failure they catch is a 400-pixel logo across half a
 * projector: invisible on the laptop it was authored on, mush from row 12.
 */
describe("image sizes", () => {
  /** A PNG header claiming a size. Nothing decodes it; only the header is read. */
  function png(width: number, height: number): Buffer {
    const bytes = Buffer.alloc(24);
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]).copy(bytes, 0);
    bytes.write("IHDR", 12, "ascii");
    bytes.writeUInt32BE(width, 16);
    bytes.writeUInt32BE(height, 20);
    return bytes;
  }

  async function buildWithLogo(markup: string, size: Buffer) {
    const root = await mkdtemp(join(tmpdir(), "slidx-assets-"));
    await mkdir(join(root, "slides"), { recursive: true });
    await writeFile(join(root, "slides", "0001.md"), markup);
    await writeFile(join(root, "slides", "logo.png"), size);

    // The rule warns rather than errors — a soft logo still shows, and
    // stopping a build minutes before a talk is how a linter gets switched
    // off — so the assertion has to read what was said, not an exit code.
    const warnings: string[] = [];
    const logger = createLogger("silent");
    logger.warn = (message: string) => void warnings.push(message);

    await build({
      root,
      logLevel: "silent",
      customLogger: logger,
      plugins: [slidx()],
      build: { outDir: join(root, "dist") },
    });

    return warnings.join("\n");
  }

  it("catches a logo drawn far wider than its own pixels", async () => {
    const output = await buildWithLogo(
      '# Results\n\n<img src="logo.png" width="1440" alt="the logo">\n',
      png(400, 200),
    );

    expect(output).toContain("resolution/upscaled");
  }, 60_000);

  it("says nothing about an image drawn at the size it is", async () => {
    const output = await buildWithLogo(
      '# Results\n\n<img src="logo.png" width="400" alt="the logo">\n',
      png(400, 200),
    );

    expect(output).not.toContain("resolution/");
  }, 60_000);
});
