/**
 * How big the deck's images actually are.
 *
 * The resolution rules — is this logo being blown up, is this chart stretched —
 * need a file's own pixel dimensions, and they run inside WebAssembly where
 * there is no filesystem to read one from. So the reading happens here and the
 * answers are handed across.
 *
 * The *parsing* still happens there. `probeImage` is the same header parser the
 * CLI uses, exposed for exactly this: a second implementation in TypeScript
 * would be a second opinion about a truncated JPEG, and the two would diverge
 * the first time somebody fixed one of them.
 *
 * Only the head of each file is read. A JPEG hides its frame header behind
 * whatever EXIF the camera wrote, so a handful of bytes is not enough; a whole
 * file is far too many, and reading forty megabytes of screenshot to learn two
 * integers is not a trade worth making on every build.
 */

import { open, readdir } from "node:fs/promises";
import { extname, join, posix, relative, sep } from "node:path";

import type { AssetSize } from "@ubugeeei/slidx-wasm";

import { probeImageHeader } from "./pipeline";

/**
 * Formats the header parser reads. Anything else is not looked at, because a
 * file it cannot answer for is silence rather than a complaint.
 */
const READABLE = new Set([".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg"]);

/**
 * Longest prefix of a file this reads. Mirrors `HEADER_BYTES` in the rule.
 */
const HEADER_BYTES = 64 * 1024;

/** Directories never worth walking for a deck's own images. */
const SKIP = new Set(["node_modules", "dist", ".git"]);

/**
 * Every readable image under the deck, keyed the way a slide writes it.
 *
 * Keyed on the path *relative to the deck's directory*, because that is what a
 * slide's `src` says and what the rule looks up. A slide that writes
 * `/logo.png` resolves against the same root, so the leading slash is dropped
 * on both sides.
 */
export async function readAssetSizes(root: string, srcDir: string): Promise<AssetSize[]> {
  const directory = join(root, srcDir);
  const files = await walk(directory);

  const sizes = await Promise.all(
    files.map(async (path) => {
      const probed = await probe(path);
      if (probed === undefined) return undefined;

      // Always `/`, never `\`: the key has to match what an author typed in
      // Markdown, and nobody writes a Windows separator in an `src`.
      const key = relative(directory, path).split(sep).join(posix.sep);

      return { ...probed, path: key };
    }),
  );

  return sizes.filter((size): size is AssetSize => size !== undefined);
}

/**
 * One file's header, or nothing.
 *
 * Every failure here is silence: a file that vanished between listing and
 * reading, one this process cannot open, a format the parser does not know.
 * None of those is the author's mistake, and a linter that complains about
 * them is one they switch off.
 */
async function probe(path: string): Promise<AssetSize | undefined> {
  try {
    const head = await readHead(path);
    return (await probeImageHeader(head)) ?? undefined;
  } catch {
    return undefined;
  }
}

async function readHead(path: string): Promise<Uint8Array> {
  const handle = await open(path, "r");

  try {
    const buffer = new Uint8Array(HEADER_BYTES);
    const { bytesRead } = await handle.read(buffer, 0, HEADER_BYTES, 0);
    return buffer.subarray(0, bytesRead);
  } finally {
    await handle.close();
  }
}

async function walk(directory: string): Promise<string[]> {
  let entries;
  try {
    entries = await readdir(directory, { withFileTypes: true });
  } catch {
    // A deck with no directory yet is the state every new project starts in.
    return [];
  }

  const found: string[] = [];

  for (const entry of entries) {
    const path = join(directory, entry.name);

    if (entry.isDirectory()) {
      if (SKIP.has(entry.name)) continue;
      found.push(...(await walk(path)));
      continue;
    }

    if (READABLE.has(extname(entry.name).toLowerCase())) found.push(path);
  }

  return found;
}

/** Exported so a test can state the cap without repeating the number. */
export const ASSET_HEADER_BYTES = HEADER_BYTES;
