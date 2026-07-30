/**
 * Frames from a real run, written as one animated image.
 *
 * A recording of a browser surface has to survive being an `<img>`: that is what
 * a README renders, and a `<video>` there is either stripped or a control nobody
 * presses. So the frames a driver captures are encoded here as an **animated
 * PNG** — the extension stays `.png`, because a served `Content-Type` decides
 * whether a browser draws an image at all and no proxy has an opinion about APNG
 * bytes.
 *
 * `scripts/record.mjs` writes webm for the documentation site and gives its
 * reason there: it is what the browser recording a page already produces.
 * Neither format replaces the other. A webm cannot go in a README, and a video
 * of a page is timed by the machine that recorded it — which is the property
 * these recordings exist to avoid.
 *
 * # Why lossless, when a GIF would be smaller
 *
 * It would not be. GIF has 256 colours a frame and LZW; this content is a
 * text-heavy interface beside a rendered slide, so a palette shows as banding
 * across every anti-aliased glyph, and deflate beats LZW on flat fills anyway.
 * The reason to reach for a palette would be size, and the sparse frames below
 * take more off the total than quantising would.
 *
 * # Why the frames are sparse
 *
 * An editor is almost entirely still. Between two frames of a drag what changes
 * is a ghost, two guide lines and one line of a file — a few per cent of the
 * pixels. APNG can say exactly that: a frame carries its own rectangle, and
 * inside it a transparent pixel composited `OVER` leaves what was already there.
 * So every frame after the first stores the bounding box of what actually
 * changed, with everything unchanged inside it made transparent, and the deflate
 * stream for the untouched majority costs almost nothing.
 *
 * Two frames that are pixel-identical are not stored twice either. The second
 * one's delay is added to the first, because a still that lasts a second is one
 * frame with a longer delay rather than ten frames of the same thing.
 *
 * # What is deterministic here, and what is not
 *
 * Every byte is a function of the pixels handed in: the rectangles, the
 * transparency, the filter chosen per scanline. Nothing reads a clock. What is
 * *not* stable across machines is the deflate stream — zlib's output depends on
 * the version Node was built against — so regenerating on a different Node major
 * rewrites the file without a pixel having moved. The content is reproducible;
 * the bytes are reproducible on one toolchain.
 *
 * A viewer that cannot animate this draws the first frame, because that frame is
 * also the file's ordinary image. The fallback is a still of the gesture rather
 * than a broken image icon.
 */

import { be32, chunk, compress, header, CHANNELS, SIGNATURE } from "./png.mjs";

/**
 * Leave the canvas as the last frame left it, and paint over it.
 *
 * The pair that makes a sparse frame mean what it says: `NONE` keeps the pixels
 * outside this frame's rectangle, and `OVER` keeps the ones inside it that this
 * frame left transparent.
 */
const DISPOSE_NONE = 0;
const BLEND_OVER = 1;
const BLEND_SOURCE = 0;

/**
 * Frames as one animated PNG that loops forever.
 *
 * @param {{pixels: Uint8Array, delay: number}[]} frames pixels are RGBA, delay is ms
 * @param {{width: number, height: number}} size
 * @returns {Buffer}
 */
export function encodeApng(frames, { width, height }) {
  const kept = fold(frames);
  if (kept.length === 0) throw new Error("no frames to encode");

  const parts = [SIGNATURE, chunk("IHDR", header(width, height))];
  // Sequence numbers run across `fcTL` and `fdAT` together, so one counter owns
  // both or a viewer rejects the file.
  let sequence = 0;
  const first = kept[0];

  // How many frames, and how many times: zero plays is forever. A README is read
  // by somebody scrolling past, so a recording that ran once and stopped would
  // be a still by the time they looked at it.
  parts.push(
    chunk("acTL", be32(kept.length, 0)),
    chunk("fcTL", control(sequence++, region(0, 0, width, height), first.delay, BLEND_SOURCE)),
    chunk("IDAT", compress(first.pixels, width, height)),
  );

  let canvas = first.pixels;

  for (const frame of kept.slice(1)) {
    const box = changed(canvas, frame.pixels, width, height);
    // Only reached for a frame that differs, since `fold` merged the ones that
    // do not — so an empty box here would be a bug rather than a still.
    if (box === undefined) throw new Error("a folded frame changed nothing");

    parts.push(
      chunk("fcTL", control(sequence++, box, frame.delay, BLEND_OVER)),
      chunk(
        "fdAT",
        Buffer.concat([
          be32(sequence++),
          compress(sparse(canvas, frame.pixels, width, box), box.width, box.height),
        ]),
      ),
    );

    canvas = frame.pixels;
  }

  parts.push(chunk("IEND", Buffer.alloc(0)));

  return Buffer.concat(parts);
}

/**
 * Consecutive identical frames as one frame that lasts longer.
 *
 * A driver holds a still by capturing the same screen again, which is the honest
 * way to author a pause — the alternative is a delay it invented. Storing them
 * is not honesty, it is bytes.
 */
function fold(frames) {
  const kept = [];

  for (const frame of frames) {
    const last = kept[kept.length - 1];
    if (last !== undefined && same(last.pixels, frame.pixels)) {
      last.delay += frame.delay;
      continue;
    }

    kept.push({ pixels: frame.pixels, delay: frame.delay });
  }

  return kept;
}

function same(one, other) {
  if (one.length !== other.length) return false;
  for (let at = 0; at < one.length; at += 1) if (one[at] !== other[at]) return false;

  return true;
}

/** The bounding box of every pixel two frames disagree about. */
function changed(before, after, width, height) {
  let left = width;
  let right = -1;
  let top = height;
  let bottom = -1;

  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      if (agree(before, after, (y * width + x) * CHANNELS)) continue;

      if (x < left) left = x;
      if (x > right) right = x;
      if (y < top) top = y;
      if (y > bottom) bottom = y;
    }
  }

  if (right < 0) return undefined;

  return region(left, top, right - left + 1, bottom - top + 1);
}

function agree(before, after, at) {
  return (
    before[at] === after[at] &&
    before[at + 1] === after[at + 1] &&
    before[at + 2] === after[at + 2] &&
    before[at + 3] === after[at + 3]
  );
}

function region(x, y, width, height) {
  return { x, y, width, height };
}

/**
 * A frame's rectangle, with every pixel it did not change made transparent.
 *
 * Composited `OVER`, this leaves the canvas alone wherever the frame agreed with
 * it — which is what makes a still interface cost nothing to keep on screen.
 */
function sparse(before, after, width, box) {
  const out = new Uint8Array(box.width * box.height * CHANNELS);

  for (let y = 0; y < box.height; y += 1) {
    for (let x = 0; x < box.width; x += 1) {
      const from = ((box.y + y) * width + box.x + x) * CHANNELS;
      if (agree(before, after, from)) continue;

      out.set(after.subarray(from, from + CHANNELS), (y * box.width + x) * CHANNELS);
    }
  }

  return out;
}

/** One frame's control chunk: where it goes, how long it stays, how it lands. */
function control(sequence, box, delay, blend) {
  const data = Buffer.alloc(26);
  data.writeUInt32BE(sequence, 0);
  data.writeUInt32BE(box.width, 4);
  data.writeUInt32BE(box.height, 8);
  data.writeUInt32BE(box.x, 12);
  data.writeUInt32BE(box.y, 16);
  // Milliseconds, written as a fraction of a second: a driver counts in
  // thousandths, and a viewer that read the numerator and ignored the
  // denominator is not a thing.
  data.writeUInt16BE(Math.round(delay), 20);
  data.writeUInt16BE(1000, 22);
  data[24] = DISPOSE_NONE;
  data[25] = blend;

  return data;
}
