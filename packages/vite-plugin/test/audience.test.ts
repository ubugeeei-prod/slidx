/**
 * Opting a deck into the audience channel at the Vite boundary.
 *
 * The Worker and the client already exist. These tests own the seam: a named
 * Worker and room becoming one bundled module, that module reaching pages only
 * when opted in, and the default deck still fetching nothing.
 */

import { mkdtemp, mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { build } from "vite";
import { describe, expect, it } from "vite-plus/test";

import { slidx } from "../src";
import { audienceClientSource, withAudienceClient } from "../src/audience";
import { resolveOptions } from "../src/options";

const CHANNEL = {
  endpoint: "https://audience.example.workers.dev",
  room: "zero-js",
};

const PAGE = `<!doctype html><html lang="en"><head></head><body><p>Hi</p></body></html>`;

describe("withAudienceClient", () => {
  it("leaves a page alone when the room slug is not one the Worker will accept", () => {
    expect(withAudienceClient(PAGE, "/audience.js", { ...CHANNEL, room: "Talk" })).toBe(PAGE);
  });

  it("marks the document and loads the client when the room is a slug", () => {
    const page = withAudienceClient(PAGE, "/audience.js", CHANNEL);

    expect(page).toContain('data-slidx-audience="{&quot;endpoint&quot;:');
    expect(page).toContain("&quot;room&quot;:&quot;zero-js&quot;");
    expect(page).toContain('<script type="module" src="/audience.js"></script>');
    expect(page).not.toContain("hostKey");
  });

  it("puts a host key on a presenter page and nowhere as a default", () => {
    const page = withAudienceClient(PAGE, "/audience.js", {
      ...CHANNEL,
      hostKey: "speaker-secret",
    });

    expect(page).toContain("&quot;hostKey&quot;:&quot;speaker-secret&quot;");
  });
});

describe("audienceClientSource", () => {
  it("is relative to the page that loads it", () => {
    expect(audienceClientSource("slides/index.html", "slides/audience.js")).toBe("./audience.js");
    expect(audienceClientSource("slides/2/index.html", "slides/audience.js")).toBe(
      "../audience.js",
    );
    expect(audienceClientSource("slides/presenter/index.html", "slides/audience.js")).toBe(
      "../audience.js",
    );
  });
});

describe("resolveOptions", () => {
  it("leaves the channel off unless both an endpoint and a room were named", () => {
    expect(resolveOptions().audience).toBeUndefined();
    expect(
      resolveOptions({ audience: { endpoint: CHANNEL.endpoint, room: "" } }).audience,
    ).toBeUndefined();
    expect(
      resolveOptions({ audience: { room: CHANNEL.room, endpoint: "  " } }).audience,
    ).toBeUndefined();
    expect(resolveOptions({ audience: CHANNEL }).audience).toEqual(CHANNEL);
  });
});

describe("a built deck", () => {
  it("emits the client only when the deck named a Worker", async () => {
    const opted = await buildDeck({ "0001.md": "# One\n" }, { audience: CHANNEL });
    const off = await buildDeck({ "0001.md": "# One\n" });

    expect(opted.files.some((file) => file.endsWith("audience.js"))).toBe(true);
    expect(off.files.some((file) => file.endsWith("audience.js"))).toBe(false);

    const slide = await readFile(join(opted.root, "dist/slides/index.html"), "utf8");
    expect(slide).toContain("data-slidx-audience=");
    expect(slide).toContain('<script type="module"');
    expect(slide).not.toContain("hostKey");

    const presenter = await readFile(join(opted.root, "dist/slides/presenter/index.html"), "utf8");
    expect(presenter).toContain("data-slidx-audience=");

    const print = await readFile(join(opted.root, "dist/slides/print/index.html"), "utf8");
    const overview = await readFile(join(opted.root, "dist/slides/overview/index.html"), "utf8");
    expect(print).not.toContain("data-slidx-audience");
    expect(overview).not.toContain("data-slidx-audience");

    const silent = await readFile(join(off.root, "dist/slides/index.html"), "utf8");
    expect(silent).not.toContain("data-slidx-audience");
    expect(silent).not.toContain("audience.js");
  }, 60_000);

  it("puts the host key only on the presenter page", async () => {
    const { root } = await buildDeck(
      { "0001.md": "# One\n" },
      { audience: { ...CHANNEL, hostKey: "speaker-secret" } },
    );

    const slide = await readFile(join(root, "dist/slides/index.html"), "utf8");
    const presenter = await readFile(join(root, "dist/slides/presenter/index.html"), "utf8");

    expect(slide).not.toContain("speaker-secret");
    expect(presenter).toContain("speaker-secret");
  }, 60_000);
});

async function buildDeck(
  slides: Record<string, string>,
  options: Parameters<typeof slidx>[0] = {},
): Promise<{ root: string; files: string[] }> {
  const root = await mkdtemp(join(tmpdir(), "slidx-audience-"));
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
  let entries;
  try {
    entries = await readdir(directory, { withFileTypes: true });
  } catch {
    return [];
  }

  const files: string[] = [];
  for (const entry of entries) {
    const path = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) files.push(...(await walk(join(directory, entry.name), path)));
    else files.push(path);
  }
  return files.sort();
}
