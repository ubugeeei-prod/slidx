/**
 * Streaming one dropped image or video into the deck's own assets directory.
 *
 * The browser supplies bytes, an encoded file name, and a media type. This
 * boundary owns path confinement, supported formats, the size cap, and
 * collision-safe naming. It never accepts a destination path from the browser.
 */

import { mkdir, open, unlink, type FileHandle } from "node:fs/promises";
import type { IncomingMessage } from "node:http";
import { extname, join } from "node:path";

export const MEDIA_UPLOAD_ROUTE = "/__slidx/media";
export const MEDIA_UPLOAD_BYTES = 256 * 1024 * 1024;

export type UploadedMediaKind = "image" | "video";

export interface UploadedMedia {
  kind: UploadedMediaKind;
  src: string;
  alt: string;
}

interface Format {
  kind: UploadedMediaKind;
  extensions: readonly string[];
}

const FORMATS = new Map<string, Format>([
  ["image/png", { kind: "image", extensions: [".png"] }],
  ["image/jpeg", { kind: "image", extensions: [".jpg", ".jpeg"] }],
  ["image/gif", { kind: "image", extensions: [".gif"] }],
  ["image/webp", { kind: "image", extensions: [".webp"] }],
  ["image/avif", { kind: "image", extensions: [".avif"] }],
  ["video/mp4", { kind: "video", extensions: [".mp4", ".m4v"] }],
  ["video/webm", { kind: "video", extensions: [".webm"] }],
  ["video/ogg", { kind: "video", extensions: [".ogv", ".ogg"] }],
  ["video/quicktime", { kind: "video", extensions: [".mov"] }],
]);

const WINDOWS_DEVICE = /^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])$/i;

export class MediaUploadError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = "MediaUploadError";
  }
}

export async function uploadMedia(
  request: IncomingMessage,
  root: string,
  srcDir: string,
  base: string,
  limit = MEDIA_UPLOAD_BYTES,
): Promise<UploadedMedia> {
  const format = formatOf(request);
  const original = originalName(request);
  const file = safeName(original, format);
  const directory = join(root, srcDir, "assets");

  assertLength(request, limit);
  await mkdir(directory, { recursive: true });

  const reserved = await reserve(directory, file.stem, file.extension);

  try {
    const bytes = await write(request, reserved.handle, limit);
    if (bytes === 0) throw new MediaUploadError(400, "The dropped file is empty.");
  } catch (error) {
    await reserved.handle.close().catch(() => {});
    await unlink(reserved.path).catch(() => {});
    throw error;
  }

  await reserved.handle.close();

  const route = [base, "assets", encodeURIComponent(reserved.name)].filter(Boolean).join("/");
  return {
    kind: format.kind,
    src: `/${route}`,
    alt: altOf(original),
  };
}

function formatOf(request: IncomingMessage): Format {
  const contentType = header(request, "content-type").split(";", 1)[0]!.trim().toLowerCase();
  const format = FORMATS.get(contentType);
  if (!format) {
    throw new MediaUploadError(
      415,
      "Drop a PNG, JPEG, GIF, WebP, AVIF, MP4, WebM, Ogg video, or QuickTime video.",
    );
  }

  return format;
}

function originalName(request: IncomingMessage): string {
  const encoded = header(request, "x-slidx-name");
  if (!encoded) throw new MediaUploadError(400, "The dropped file has no name.");

  let name: string;
  try {
    name = decodeURIComponent(encoded).normalize("NFC").trim();
  } catch {
    throw new MediaUploadError(400, "The dropped file name is not valid URI text.");
  }

  if (!name || name === "." || name === ".." || name.includes("/") || name.includes("\\"))
    throw new MediaUploadError(400, "The dropped file name must not contain a path.");

  if (name.includes("\0")) throw new MediaUploadError(400, "The dropped file name is not valid.");
  return name;
}

function safeName(original: string, format: Format): { stem: string; extension: string } {
  const supplied = extname(original).toLowerCase();
  const extension = format.extensions.includes(supplied) ? supplied : format.extensions[0]!;
  const withoutExtension = supplied ? original.slice(0, -supplied.length) : original;
  let stem = withoutExtension
    .normalize("NFKC")
    .replace(/[^\p{Letter}\p{Number}._-]+/gu, "-")
    .replace(/^[.\s_-]+|[.\s_-]+$/g, "")
    .slice(0, 96);

  if (!stem) stem = format.kind;
  if (WINDOWS_DEVICE.test(stem)) stem = `${format.kind}-${stem}`;

  return { stem, extension };
}

function altOf(original: string): string {
  const extension = extname(original);
  return (extension ? original.slice(0, -extension.length) : original)
    .replace(/[-_]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function assertLength(request: IncomingMessage, limit: number): void {
  const value = header(request, "content-length");
  if (!value) return;

  const length = Number(value);
  if (!Number.isSafeInteger(length) || length < 0)
    throw new MediaUploadError(400, "The dropped file size is not valid.");
  if (length > limit) throw tooLarge(limit);
}

async function reserve(
  directory: string,
  stem: string,
  extension: string,
): Promise<{ handle: FileHandle; name: string; path: string }> {
  for (let index = 1; index <= 10_000; index += 1) {
    const suffix = index === 1 ? "" : `-${index}`;
    const name = `${stem}${suffix}${extension}`;
    const path = join(directory, name);

    try {
      return { handle: await open(path, "wx"), name, path };
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
    }
  }

  throw new MediaUploadError(409, "No unused name is available for the dropped file.");
}

async function write(request: IncomingMessage, handle: FileHandle, limit: number): Promise<number> {
  let written = 0;

  for await (const part of request) {
    const bytes = Buffer.isBuffer(part) ? part : Buffer.from(part as Uint8Array);
    written += bytes.byteLength;
    if (written > limit) throw tooLarge(limit);
    await writeAll(handle, bytes);
  }

  return written;
}

async function writeAll(handle: FileHandle, bytes: Buffer): Promise<void> {
  let offset = 0;
  while (offset < bytes.byteLength) {
    const result = await handle.write(bytes, offset, bytes.byteLength - offset);
    offset += result.bytesWritten;
  }
}

function header(request: IncomingMessage, name: string): string {
  const value = request.headers[name];
  return Array.isArray(value) ? (value[0] ?? "") : (value ?? "");
}

function tooLarge(limit: number): MediaUploadError {
  return new MediaUploadError(
    413,
    `The dropped file is larger than ${Math.floor(limit / 1024 / 1024)} MiB.`,
  );
}
