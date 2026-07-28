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
import { beforeAll, describe, expect, it } from "vitest";

import { slidx } from "../src/index";

let outDir: string;

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

  it("ships no JavaScript", () => {
    // The property that makes a deck load instantly on venue Wi-Fi. A stray
    // empty chunk from the virtual entry would break it silently.
    expect(result.files.filter((file) => file.endsWith(".js"))).toEqual([]);
  });

  it("emits nothing but the slides", () => {
    expect(result.files).toEqual(["slides/2/index.html", "slides/index.html"]);
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
    expect(files).toEqual(["index.html"]);
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
