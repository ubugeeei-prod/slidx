/**
 * PNG, as much of it as a recording needs: reading a screenshot and writing a
 * picture back.
 *
 * A dependency would be the obvious thing here, and it is not the right thing
 * for what these two are. Reading is one shape of file — what a browser writes
 * when it is asked for a screenshot — and writing is one shape of file too,
 * because [`animate`](./animate.mjs) chooses every byte of the layout it needs.
 * A library would carry the other twenty cases, and the two things this actually
 * has to get right, per-scanline filtering and CRCs, are the two a library would
 * hide.
 *
 * Nothing here knows what an animation is. That is the reason this file exists
 * next to `animate.mjs` rather than inside it: this one knows the container, and
 * that one knows what a recording of an editor is.
 */

import { deflateSync, inflateSync } from "node:zlib";

/** PNG's own eight bytes, which say what the file is before any chunk does. */
export const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

/** RGBA, which is what a screenshot is and what a sparse frame needs. */
export const CHANNELS = 4;

/**
 * A screenshot as pixels.
 *
 * Only what a browser writes: eight bits a channel, RGB or RGBA, not
 * interlaced. Anything else throws rather than being guessed at — a decoder that
 * quietly mis-read a bit depth would produce an animation in the wrong colours,
 * which is the failure that looks like a design decision.
 *
 * @param {Buffer} file
 * @returns {{width: number, height: number, pixels: Uint8Array}}
 */
export function decodePng(file) {
  if (!file.subarray(0, 8).equals(SIGNATURE)) throw new Error("not a PNG");

  let header;
  const parts = [];

  for (let at = 8; at + 8 <= file.length; ) {
    const length = file.readUInt32BE(at);
    const type = file.toString("ascii", at + 4, at + 8);
    const data = file.subarray(at + 8, at + 8 + length);
    at += 12 + length;

    if (type === "IHDR") header = readHeader(data);
    else if (type === "IDAT") parts.push(data);
    else if (type === "IEND") break;
  }

  if (header === undefined) throw new Error("PNG with no IHDR");

  const { width, height, channels } = header;
  const raw = inflateSync(Buffer.concat(parts));
  const scanline = width * channels;
  const pixels = new Uint8Array(width * height * CHANNELS);
  // Reused between rows: unfiltering reads the row above, which has already
  // been written back over its own filtered bytes.
  const above = new Uint8Array(scanline);
  const row = new Uint8Array(scanline);

  for (let y = 0; y < height; y += 1) {
    const start = y * (scanline + 1);
    row.set(raw.subarray(start + 1, start + 1 + scanline));
    unfilter(raw[start], row, above, channels);

    for (let x = 0; x < width; x += 1) {
      const from = x * channels;
      const to = (y * width + x) * CHANNELS;
      pixels[to] = row[from];
      pixels[to + 1] = row[from + 1];
      pixels[to + 2] = row[from + 2];
      pixels[to + 3] = channels === 4 ? row[from + 3] : 0xff;
    }

    above.set(row);
  }

  return { width, height, pixels };
}

function readHeader(data) {
  const depth = data[8];
  const colour = data[9];
  const interlace = data[12];

  if (depth !== 8) throw new Error(`PNG bit depth ${depth}, and only 8 is read here`);
  if (colour !== 2 && colour !== 6) throw new Error(`PNG colour type ${colour} is not RGB or RGBA`);
  if (interlace !== 0) throw new Error("interlaced PNG");

  return {
    width: data.readUInt32BE(0),
    height: data.readUInt32BE(4),
    channels: colour === 6 ? 4 : 3,
  };
}

/** PNG's five reconstruction filters, in place. */
function unfilter(filter, row, above, channels) {
  for (let at = 0; at < row.length; at += 1) {
    const left = at >= channels ? row[at - channels] : 0;
    const up = above[at];
    const corner = at >= channels ? above[at - channels] : 0;

    switch (filter) {
      case 0:
        break;
      case 1:
        row[at] = (row[at] + left) & 0xff;
        break;
      case 2:
        row[at] = (row[at] + up) & 0xff;
        break;
      case 3:
        row[at] = (row[at] + ((left + up) >> 1)) & 0xff;
        break;
      case 4:
        row[at] = (row[at] + paeth(left, up, corner)) & 0xff;
        break;
      default:
        throw new Error(`PNG filter ${filter}`);
    }
  }
}

function paeth(left, up, corner) {
  const estimate = left + up - corner;
  const toLeft = Math.abs(estimate - left);
  const toUp = Math.abs(estimate - up);
  const toCorner = Math.abs(estimate - corner);

  if (toLeft <= toUp && toLeft <= toCorner) return left;
  return toUp <= toCorner ? up : corner;
}

/** The `IHDR` for an image of RGBA pixels. */
export function header(width, height) {
  const data = Buffer.alloc(13);
  data.writeUInt32BE(width, 0);
  data.writeUInt32BE(height, 4);
  // Eight bits a channel, RGBA, deflate, adaptive filtering, not interlaced.
  data.set([8, 6, 0, 0, 0], 8);

  return data;
}

/**
 * One image's pixels as a deflate stream, filtered a scanline at a time.
 *
 * The filter is chosen per row by the sum of its output read as signed bytes,
 * which is the heuristic the PNG specification itself suggests and is worth
 * roughly a third of the file here. It has to be chosen rather than fixed: a row
 * of a sparse frame is mostly transparent, where `None` costs nothing, and a row
 * of a rendered slide is where `Paeth` costs a great deal less.
 *
 * @param {Uint8Array} pixels RGBA
 * @returns {Buffer}
 */
export function compress(pixels, width, height) {
  const scanline = width * CHANNELS;
  const raw = Buffer.alloc((scanline + 1) * height);
  const above = new Uint8Array(scanline);
  const row = new Uint8Array(scanline);
  const candidate = new Uint8Array(scanline);

  for (let line = 0; line < height; line += 1) {
    row.set(pixels.subarray(line * scanline, (line + 1) * scanline));

    let bestFilter = 0;
    let bestCost = Infinity;
    let best = null;

    for (const filter of [0, 1, 2, 3, 4]) {
      apply(filter, row, above, candidate);
      const cost = deviation(candidate);
      if (cost < bestCost) {
        bestCost = cost;
        bestFilter = filter;
        best = candidate.slice();
      }
    }

    raw[line * (scanline + 1)] = bestFilter;
    raw.set(best, line * (scanline + 1) + 1);
    above.set(row);
  }

  return deflateSync(raw, { level: 9 });
}

/** One of PNG's filters, forward. */
function apply(filter, row, above, out) {
  for (let at = 0; at < row.length; at += 1) {
    const left = at >= CHANNELS ? row[at - CHANNELS] : 0;
    const up = above[at];
    const corner = at >= CHANNELS ? above[at - CHANNELS] : 0;

    switch (filter) {
      case 0:
        out[at] = row[at];
        break;
      case 1:
        out[at] = (row[at] - left) & 0xff;
        break;
      case 2:
        out[at] = (row[at] - up) & 0xff;
        break;
      case 3:
        out[at] = (row[at] - ((left + up) >> 1)) & 0xff;
        break;
      default:
        out[at] = (row[at] - paeth(left, up, corner)) & 0xff;
    }
  }
}

/** How far a filtered row is from flat, counting a byte as signed. */
function deviation(row) {
  let total = 0;
  for (const byte of row) total += byte < 128 ? byte : 256 - byte;

  return total;
}

/** One chunk: its length, its type, its data, and the CRC of the last two. */
export function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);

  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body), 0);

  return Buffer.concat([length, body, crc]);
}

/** Some numbers, big-endian, which is how every field in a PNG is written. */
export function be32(...values) {
  const data = Buffer.alloc(4 * values.length);
  for (const [at, value] of values.entries()) data.writeUInt32BE(value, at * 4);

  return data;
}

/** PNG's CRC-32, built once. */
const CRC_TABLE = (() => {
  const table = new Uint32Array(256);

  for (let at = 0; at < 256; at += 1) {
    let value = at;
    for (let bit = 0; bit < 8; bit += 1) {
      value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
    }
    table[at] = value >>> 0;
  }

  return table;
})();

function crc32(data) {
  let value = 0xffffffff;
  for (const byte of data) value = CRC_TABLE[(value ^ byte) & 0xff] ^ (value >>> 8);

  return (value ^ 0xffffffff) >>> 0;
}
