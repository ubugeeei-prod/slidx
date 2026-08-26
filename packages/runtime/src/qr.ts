/**
 * A QR code, drawn here so a pairing URL never leaves the presenter.
 *
 * slidx already has an encoder in Rust for codes that are known at build
 * time. A pairing is minted when the presenter opens, so the same job has
 * to run in this window. The tables are the ISO/IEC 18004 ones
 * `slidx_qr` uses; the output is the same SVG contract — `currentColor`,
 * no pixel size, nothing fetched.
 *
 * Versions 1 through 10, byte mode, medium correction. A pairing URL is
 * well under a hundred bytes. Past version 10 the modules are too fine
 * for a phone to resolve from a laptop across a lectern anyway.
 */

const MAX_VERSION = 10;
const MIN_QUIET = 4;

/** Capacities in payload bytes at medium correction, versions 1–10. */
const BYTE_CAPACITY_M = [0, 14, 26, 42, 62, 84, 106, 122, 152, 180, 213];

/**
 * Block layout at medium: [eccPerBlock, g1Blocks, g1Data, g2Blocks, g2Data].
 * Transcribed from ISO/IEC 18004 table 9.
 */
const LAYOUT_M: Array<readonly [number, number, number, number, number]> = [
  [0, 0, 0, 0, 0],
  [10, 1, 16, 0, 0],
  [16, 1, 28, 0, 0],
  [26, 1, 44, 0, 0],
  [18, 2, 32, 0, 0],
  [24, 2, 43, 0, 0],
  [16, 4, 27, 0, 0],
  [18, 4, 31, 0, 0],
  [22, 2, 38, 2, 39],
  [22, 3, 36, 2, 37],
  [26, 4, 43, 1, 44],
];

const ALIGNMENT: readonly (readonly number[])[] = [
  [],
  [],
  [6, 18],
  [6, 22],
  [6, 26],
  [6, 30],
  [6, 34],
  [6, 22, 38],
  [6, 24, 42],
  [6, 26, 46],
  [6, 28, 50],
];

const EXP = new Uint8Array(256);
const LOG = new Uint8Array(256);

(() => {
  let value = 1;
  for (let i = 0; i < 255; i += 1) {
    EXP[i] = value;
    LOG[value] = i;
    value <<= 1;
    if (value & 0x100) value ^= 0x11d;
  }
})();

function gfMul(a: number, b: number): number {
  if (a === 0 || b === 0) return 0;
  return EXP[(LOG[a] + LOG[b]) % 255] ?? 0;
}

function rsGenerator(degree: number): Uint8Array {
  const generator = new Uint8Array(degree + 1);
  generator[0] = 1;
  for (let i = 0; i < degree; i += 1) {
    const next = EXP[i] ?? 0;
    for (let j = i; j >= 0; j -= 1) {
      generator[j + 1] ^= gfMul(generator[j] ?? 0, next);
    }
  }
  return generator;
}

function rsEncode(data: Uint8Array, eccCount: number): Uint8Array {
  const generator = rsGenerator(eccCount);
  const ecc = new Uint8Array(eccCount);
  for (const byte of data) {
    const factor = byte ^ (ecc[0] ?? 0);
    ecc.copyWithin(0, 1);
    ecc[eccCount - 1] = 0;
    for (let i = 0; i < eccCount; i += 1) {
      ecc[i] ^= gfMul(generator[i + 1] ?? 0, factor);
    }
  }
  return ecc;
}

function versionOf(bytes: number): number | null {
  for (let version = 1; version <= MAX_VERSION; version += 1) {
    if (bytes <= (BYTE_CAPACITY_M[version] ?? 0)) return version;
  }
  return null;
}

function sizeOf(version: number): number {
  return 4 * version + 17;
}

function dataCodewords(version: number): number {
  const layout = LAYOUT_M[version];
  if (!layout) return 0;
  return layout[1] * layout[2] + layout[3] * layout[4];
}

function payloadBits(text: string, version: number): number[] {
  const bytes = new TextEncoder().encode(text);
  const countBits = version <= 9 ? 8 : 16;
  const bits: number[] = [0, 1, 0, 0];
  pushBits(bits, bytes.length, countBits);
  for (const byte of bytes) pushBits(bits, byte, 8);

  const capacity = dataCodewords(version) * 8;
  const terminator = Math.min(4, capacity - bits.length);
  for (let i = 0; i < terminator; i += 1) bits.push(0);
  while (bits.length % 8 !== 0 && bits.length < capacity) bits.push(0);

  const pad = [0b11101100, 0b00010001];
  let padIndex = 0;
  while (bits.length < capacity) {
    pushBits(bits, pad[padIndex % 2] ?? 0, 8);
    padIndex += 1;
  }

  return bits;
}

function pushBits(bits: number[], value: number, width: number): void {
  for (let i = width - 1; i >= 0; i -= 1) bits.push((value >> i) & 1);
}

function bitsToBytes(bits: number[]): Uint8Array {
  const bytes = new Uint8Array(bits.length / 8);
  for (let i = 0; i < bytes.length; i += 1) {
    let value = 0;
    for (let bit = 0; bit < 8; bit += 1) value = (value << 1) | (bits[i * 8 + bit] ?? 0);
    bytes[i] = value;
  }
  return bytes;
}

function interleave(version: number, data: Uint8Array): Uint8Array {
  const [eccPerBlock, g1Blocks, g1Data, g2Blocks, g2Data] = LAYOUT_M[version] ?? [0, 0, 0, 0, 0];
  const blocks: Array<{ data: Uint8Array; ecc: Uint8Array }> = [];
  let offset = 0;

  for (let i = 0; i < g1Blocks; i += 1) {
    const block = data.subarray(offset, offset + g1Data);
    offset += g1Data;
    blocks.push({ data: block, ecc: rsEncode(block, eccPerBlock) });
  }
  for (let i = 0; i < g2Blocks; i += 1) {
    const block = data.subarray(offset, offset + g2Data);
    offset += g2Data;
    blocks.push({ data: block, ecc: rsEncode(block, eccPerBlock) });
  }

  const out: number[] = [];
  const maxData = Math.max(g1Data, g2Data);
  for (let i = 0; i < maxData; i += 1) {
    for (const block of blocks) {
      if (i < block.data.length) out.push(block.data[i] ?? 0);
    }
  }
  for (let i = 0; i < eccPerBlock; i += 1) {
    for (const block of blocks) out.push(block.ecc[i] ?? 0);
  }
  return new Uint8Array(out);
}

type Module = boolean | null;

function matrixOf(version: number): Module[][] {
  const size = sizeOf(version);
  const grid: Module[][] = Array.from({ length: size }, () => Array<Module>(size).fill(null));

  const finder = (row: number, column: number) => {
    for (let r = -1; r <= 7; r += 1) {
      for (let c = -1; c <= 7; c += 1) {
        const rr = row + r;
        const cc = column + c;
        if (rr < 0 || cc < 0 || rr >= size || cc >= size) continue;
        const on = r >= 0 && r <= 6 && c >= 0 && c <= 6 && (r === 0 || r === 6 || c === 0 || c === 6 || (r >= 2 && r <= 4 && c >= 2 && c <= 4));
        const cell = grid[rr];
        if (cell) cell[cc] = on;
      }
    }
  };

  finder(0, 0);
  finder(0, size - 7);
  finder(size - 7, 0);

  for (let i = 8; i < size - 8; i += 1) {
    const row = grid[6];
    const col = grid[i];
    if (row) row[i] = i % 2 === 0;
    if (col) col[6] = i % 2 === 0;
  }

  const centers = ALIGNMENT[version] ?? [];
  for (const row of centers) {
    for (const column of centers) {
      if ((row === 6 && column === 6) || (row === 6 && column === size - 7) || (row === size - 7 && column === 6)) {
        continue;
      }
      for (let r = -2; r <= 2; r += 1) {
        for (let c = -2; c <= 2; c += 1) {
          const cell = grid[row + r];
          if (cell) cell[column + c] = Math.max(Math.abs(r), Math.abs(c)) !== 1;
        }
      }
    }
  }

  if (version >= 7) {
    const bits = versionInformation(version);
    for (let i = 0; i < 18; i += 1) {
      const bit = ((bits >> i) & 1) === 1;
      const a = Math.floor(i / 3);
      const b = i % 3;
      const left = grid[size - 11 + b];
      const top = grid[a];
      if (left) left[a] = bit;
      if (top) top[size - 11 + b] = bit;
    }
  }

  const dark = grid[size - 8];
  if (dark) dark[8] = true;

  return grid;
}

function versionInformation(version: number): number {
  let bits = version << 12;
  const generator = 0x1f25;
  for (let i = 17; i >= 12; i -= 1) {
    if (bits & (1 << i)) bits ^= generator << (i - 12);
  }
  return (version << 12) | bits;
}

function formatBits(mask: number): number {
  const data = (0b00 << 3) | mask;
  let bits = data << 10;
  const generator = 0b10100110111;
  for (let i = 14; i >= 10; i -= 1) {
    if (bits & (1 << i)) bits ^= generator << (i - 10);
  }
  return (data << 10 | bits) ^ 0b101010000010010;
}

function writeFormat(grid: Module[][], mask: number): void {
  const size = grid.length;
  const bits = formatBits(mask);
  for (let i = 0; i < 15; i += 1) {
    const bit = ((bits >> i) & 1) === 1;
    if (i < 6) {
      const row = grid[i];
      if (row) row[8] = bit;
    } else if (i < 8) {
      const row = grid[i + 1];
      if (row) row[8] = bit;
    } else {
      const row = grid[8];
      if (row) row[size - 15 + i] = bit;
    }

    if (i < 8) {
      const row = grid[8];
      if (row) row[size - i - 1] = bit;
    } else if (i === 8) {
      const row = grid[8];
      if (row) row[7] = bit;
    } else {
      const row = grid[14 - i];
      if (row) row[8] = bit;
    }
  }
}

function maskBit(mask: number, row: number, column: number): boolean {
  switch (mask) {
    case 0:
      return (row + column) % 2 === 0;
    case 1:
      return row % 2 === 0;
    case 2:
      return column % 3 === 0;
    case 3:
      return (row + column) % 3 === 0;
    case 4:
      return (Math.floor(row / 2) + Math.floor(column / 3)) % 2 === 0;
    case 5:
      return ((row * column) % 2) + ((row * column) % 3) === 0;
    case 6:
      return (((row * column) % 2) + ((row * column) % 3)) % 2 === 0;
    default:
      return (((row + column) % 2) + ((row * column) % 3)) % 2 === 0;
  }
}

function place(grid: Module[][], codewords: Uint8Array, mask: number): boolean[][] {
  const size = grid.length;
  const reserved = grid.map((row) => row.map((cell) => cell !== null));
  const bits: number[] = [];
  for (const byte of codewords) {
    for (let i = 7; i >= 0; i -= 1) bits.push((byte >> i) & 1);
  }

  let index = 0;
  let upward = true;
  for (let column = size - 1; column > 0; column -= 2) {
    if (column === 6) column -= 1;
    for (let i = 0; i < size; i += 1) {
      const row = upward ? size - 1 - i : i;
      for (const offset of [0, 1]) {
        const c = column - offset;
        if (reserved[row]?.[c]) continue;
        const bit = bits[index] ?? 0;
        index += 1;
        const dark = bit === 1 !== maskBit(mask, row, c);
        const cell = grid[row];
        if (cell) cell[c] = dark;
      }
    }
    upward = !upward;
  }

  writeFormat(grid, mask);
  return grid.map((row) => row.map((cell) => cell === true));
}

function penalty(grid: boolean[][]): number {
  const size = grid.length;
  let score = 0;

  const runScore = (run: number) => (run >= 5 ? 3 + (run - 5) : 0);

  for (let row = 0; row < size; row += 1) {
    let run = 1;
    for (let column = 1; column < size; column += 1) {
      if (grid[row]?.[column] === grid[row]?.[column - 1]) run += 1;
      else {
        score += runScore(run);
        run = 1;
      }
    }
    score += runScore(run);
  }

  for (let column = 0; column < size; column += 1) {
    let run = 1;
    for (let row = 1; row < size; row += 1) {
      if (grid[row]?.[column] === grid[row - 1]?.[column]) run += 1;
      else {
        score += runScore(run);
        run = 1;
      }
    }
    score += runScore(run);
  }

  for (let row = 0; row < size - 1; row += 1) {
    for (let column = 0; column < size - 1; column += 1) {
      const a = grid[row]?.[column];
      if (a === grid[row]?.[column + 1] && a === grid[row + 1]?.[column] && a === grid[row + 1]?.[column + 1]) {
        score += 3;
      }
    }
  }

  let dark = 0;
  for (const row of grid) {
    for (const cell of row) if (cell) dark += 1;
  }
  const percent = Math.floor((dark * 100) / (size * size));
  score += Math.floor(Math.abs(percent - 50) / 5) * 10;

  return score;
}

function encodeModules(text: string): boolean[][] | null {
  if (text === "") return null;
  const bytes = new TextEncoder().encode(text).length;
  const version = versionOf(bytes);
  if (version === null) return null;

  const data = bitsToBytes(payloadBits(text, version));
  const codewords = interleave(version, data);

  let best: boolean[][] | null = null;
  let bestScore = Infinity;
  for (let mask = 0; mask < 8; mask += 1) {
    const modules = place(matrixOf(version), codewords, mask);
    const score = penalty(modules);
    if (score < bestScore) {
      best = modules;
      bestScore = score;
    }
  }

  return best;
}

function pathData(modules: boolean[][], quiet: number): string {
  let data = "";
  for (let row = 0; row < modules.length; row += 1) {
    let column = 0;
    while (column < modules.length) {
      if (!modules[row]?.[column]) {
        column += 1;
        continue;
      }
      const start = column;
      while (column < modules.length && modules[row]?.[column]) column += 1;
      const length = column - start;
      data += `M${start + quiet} ${row + quiet}h${length}v1h-${length}z`;
    }
  }
  return data;
}

/**
 * An SVG of the pairing URL, or nothing when it will not encode.
 *
 * `null` rather than a broken mark: a code that does not scan and a missing
 * code are the same to the speaker; a wrong one is worse.
 */
export function renderQrSvg(text: string): string | null {
  const modules = encodeModules(text);
  if (modules === null) return null;

  const quiet = MIN_QUIET;
  const extent = modules.length + quiet * 2;
  const path = pathData(modules, quiet);

  return (
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${extent} ${extent}" ` +
    `shape-rendering="crispEdges" role="img">` +
    `<title>Pairing link</title>` +
    `<path fill="currentColor" d="${path}"/>` +
    `</svg>`
  );
}
