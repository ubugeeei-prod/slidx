/**
 * Opting a deck into the phone remote at the Vite boundary.
 *
 * The pairing constructors and the Worker already exist. These tests own
 * the seam: a named Worker becoming a page and a module, those reaching
 * the presenter and the phone only when opted in, and the default deck
 * still fetching nothing.
 */

import { mkdtemp, mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { build } from "vite";
import { describe, expect, it } from "vite-plus/test";

import { withRemoteClient } from "../src/remote";
import { resolveOptions, type SlidxOptions } from "../src/options";

const RELAY = { endpoint: "https://audience.example.workers.dev" };

const PAGE = `<!doctype html><html lang="en"><head></head><body><p>Hi</p></body></html>`;

describe("withRemoteClient", () => {
  it("leaves a page alone when the endpoint is empty", () => {
    expect(withRemoteClient(PAGE, { endpoint: "  " })).toBe(PAGE);
  });

  it("marks the document with the Worker origin and never a secret", () => {
    const page = withRemoteClient(PAGE, RELAY);

    expect(page).toContain('data-slidx-remote="{&quot;endpoint&quot;:');
    expect(page).toContain("audience.example.workers.dev");
    expect(page).not.toContain("secret");
    expect(page).not.toContain("<script");
  });
});

describe("resolveOptions", () => {
  it("leaves the remote off unless an endpoint was named", () => {
    expect(resolveOptions().remote).toBeUndefined();
    expect(resolveOptions({ remote: { endpoint: "  " } }).remote).toBeUndefined();
    expect(resolveOptions({ remote: RELAY }).remote).toEqual(RELAY);
  });
});

describe("a built deck", () => {
  it("emits the pairing page only when the deck named a Worker", async () => {
    const opted = await buildDeck({ "0001.md": "# One\n" }, { remote: RELAY });
    const off = await buildDeck({ "0001.md": "# One\n" });

    expect(opted.files).toContain("slides/remote.js");
    expect(opted.files).toContain("slides/remote/index.html");
    expect(off.files).not.toContain("slides/remote.js");
    expect(off.files).not.toContain("slides/remote/index.html");

    const presenter = await readFile(join(opted.root, "dist/slides/presenter/index.html"), "utf8");
    expect(presenter).toContain("data-slidx-remote=");
    expect(presenter).toContain("joinRemote");
    expect(presenter).not.toContain(RELAY.endpoint.split("https://")[1] + "/sessions");

    const phone = await readFile(join(opted.root, "dist/slides/remote/index.html"), "utf8");
    expect(phone).toContain("data-slidx-remote=");
    expect(phone).toContain("readPairing(location.href)");
    expect(phone).toContain("joinRemote");

    const slide = await readFile(join(opted.root, "dist/slides/index.html"), "utf8");
    const print = await readFile(join(opted.root, "dist/slides/print/index.html"), "utf8");
    const overview = await readFile(join(opted.root, "dist/slides/overview/index.html"), "utf8");
    expect(slide).not.toContain("data-slidx-remote");
    expect(print).not.toContain("data-slidx-remote");
    expect(overview).not.toContain("data-slidx-remote");

    const silent = await readFile(join(off.root, "dist/slides/presenter/index.html"), "utf8");
    expect(silent).not.toContain("data-slidx-remote");
    expect(silent).not.toContain("joinRemote");
    expect(silent).not.toContain("remote.js");
  }, 60_000);
});

async function buildDeck(
  slides: Record<string, string>,
  options: SlidxOptions = {},
): Promise<{ root: string; files: string[] }> {
  const { slidx } = await import("../src");
  const root = await mkdtemp(join(tmpdir(), "slidx-remote-"));
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
