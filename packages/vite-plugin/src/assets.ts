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

import { createReadStream } from "node:fs";
import { open, readFile, readdir, realpath, stat } from "node:fs/promises";
import type { IncomingMessage, ServerResponse } from "node:http";
import { extname, join, posix, relative, sep } from "node:path";

import type { AssetSize } from "@slidxjs/wasm";

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

const MEDIA_TYPES = new Map([
  [".png", "image/png"],
  [".jpg", "image/jpeg"],
  [".jpeg", "image/jpeg"],
  [".gif", "image/gif"],
  [".webp", "image/webp"],
  [".avif", "image/avif"],
  [".svg", "image/svg+xml"],
  [".mp4", "video/mp4"],
  [".m4v", "video/mp4"],
  [".webm", "video/webm"],
  [".ogv", "video/ogg"],
  [".ogg", "video/ogg"],
  [".mov", "video/quicktime"],
  [".woff", "font/woff"],
  [".woff2", "font/woff2"],
]);

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

/** One deck-owned asset ready for Rollup's output. */
export interface DeckAsset {
  fileName: string;
  source: Buffer;
}

/**
 * Reads every file under the deck's `assets` directory for a production build.
 *
 * The emitted route is the same one dev serves and the upload route returns.
 * Symlinks and dotfiles stay out: a deck asset must be an ordinary file inside
 * the directory, never an indirect read of somewhere else on the machine.
 */
export async function readDeckAssets(
  root: string,
  srcDir: string,
  base: string,
): Promise<DeckAsset[]> {
  const directory = join(root, srcDir, "assets");
  const files = await walkAssets(directory);

  return Promise.all(
    files.map(async (path) => {
      const name = relative(directory, path).split(sep).join(posix.sep);
      const fileName = [base, "assets", name].filter(Boolean).join("/");
      return { fileName, source: await readFile(path) };
    }),
  );
}

/**
 * Serves a deck-owned asset in dev, including one byte range for video seek.
 *
 * This mapping matters when `srcDir` and the public deck `base` differ. Vite
 * can serve `/slides/assets/x` from a default project by coincidence; this is
 * the explicit route that keeps every configuration and the build identical.
 */
export async function serveDeckAsset(
  request: IncomingMessage,
  response: ServerResponse,
  root: string,
  srcDir: string,
  base: string,
): Promise<boolean> {
  if (request.method !== "GET" && request.method !== "HEAD") return false;

  const parts = requestedAsset(request.url ?? "", base);
  if (!parts) return false;

  const directory = join(root, srcDir, "assets");
  const path = join(directory, ...parts);
  const resolved = await Promise.all([realpath(directory), realpath(path)]).catch(() => undefined);
  if (!resolved) return false;
  const [owner, target] = resolved;
  if (!target.startsWith(`${owner}${sep}`)) return false;

  const info = await stat(target).catch(() => undefined);
  if (!info?.isFile()) return false;

  const range = byteRange(header(request, "range"), info.size);
  response.setHeader("content-type", mediaType(path));
  response.setHeader("cache-control", "no-store");
  response.setHeader("accept-ranges", "bytes");

  if (range === "invalid") {
    response.statusCode = 416;
    response.setHeader("content-range", `bytes */${info.size}`);
    response.end();
    return true;
  }

  const start = range?.start ?? 0;
  const end = range?.end ?? Math.max(0, info.size - 1);
  const length = info.size === 0 ? 0 : end - start + 1;
  response.statusCode = range ? 206 : 200;
  response.setHeader("content-length", String(length));
  if (range) response.setHeader("content-range", `bytes ${start}-${end}/${info.size}`);

  if (request.method === "HEAD" || info.size === 0) {
    response.end();
    return true;
  }

  createReadStream(target, { start, end })
    .on("error", () => response.destroy())
    .pipe(response);
  return true;
}

async function walkAssets(directory: string): Promise<string[]> {
  const entries = await readdir(directory, { withFileTypes: true }).catch(() => []);
  const found: string[] = [];

  for (const entry of entries) {
    if (entry.name.startsWith(".")) continue;
    const path = join(directory, entry.name);
    if (entry.isDirectory()) found.push(...(await walkAssets(path)));
    if (entry.isFile()) found.push(path);
  }

  return found;
}

function requestedAsset(url: string, base: string): string[] | undefined {
  const prefix = base ? `/${base}/assets/` : "/assets/";
  const pathname = new URL(url, "http://deck.invalid").pathname;
  if (!pathname.startsWith(prefix)) return undefined;

  try {
    const parts = pathname
      .slice(prefix.length)
      .split("/")
      .map((part) => decodeURIComponent(part));
    return parts.length > 0 &&
      parts.every(
        (part) =>
          part.length > 0 &&
          part !== "." &&
          part !== ".." &&
          !part.startsWith(".") &&
          !part.includes("/") &&
          !part.includes("\\") &&
          !part.includes("\0"),
      )
      ? parts
      : undefined;
  } catch {
    return undefined;
  }
}

type Range = { start: number; end: number } | "invalid" | undefined;

function byteRange(value: string, size: number): Range {
  if (!value) return undefined;
  const match = /^bytes=(\d*)-(\d*)$/.exec(value);
  if (!match || size === 0) return "invalid";

  const [, first = "", last = ""] = match;
  if (!first && !last) return "invalid";

  if (!first) {
    const suffix = Number(last);
    if (!Number.isSafeInteger(suffix) || suffix <= 0) return "invalid";
    return { start: Math.max(0, size - suffix), end: size - 1 };
  }

  const start = Number(first);
  const askedEnd = last ? Number(last) : size - 1;
  if (
    !Number.isSafeInteger(start) ||
    !Number.isSafeInteger(askedEnd) ||
    start < 0 ||
    start >= size ||
    askedEnd < start
  )
    return "invalid";

  return { start, end: Math.min(askedEnd, size - 1) };
}

function header(request: IncomingMessage, name: string): string {
  const value = request.headers[name];
  return Array.isArray(value) ? (value[0] ?? "") : (value ?? "");
}

function mediaType(path: string): string {
  return MEDIA_TYPES.get(extname(path).toLowerCase()) ?? "application/octet-stream";
}
