/**
 * The product tours are generated evidence, but they are also shipped files.
 * Keep both halves honest: the README preview must really animate and the
 * documentation recording must really be a WebM container rather than a
 * renamed or empty capture.
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vite-plus/test";

const ROOT = join(import.meta.dirname, "../..");
const DOCS = readFileSync(join(ROOT, "docs/content/index.md"), "utf8");
const CLI = readFileSync(join(ROOT, "docs/content/cli.md"), "utf8");

describe("the generated product tours", () => {
  for (const [name, page] of [
    ["editor-tour", DOCS],
    ["cli-tour", CLI],
  ]) {
    it(`${name} has an animated preview and a playable recording`, () => {
      const preview = readFileSync(join(ROOT, `docs/media/${name}.png`));
      const recording = readFileSync(join(ROOT, `docs/media/${name}.webm`));

      expect(preview.subarray(1, 4).toString("ascii")).toBe("PNG");
      expect(preview.includes(Buffer.from("acTL"))).toBe(true);
      expect(recording.subarray(0, 4)).toEqual(Buffer.from([0x1a, 0x45, 0xdf, 0xa3]));
      expect(recording.length).toBeGreaterThan(100_000);
      expect(page).toContain(`../media/${name}.webm`);
    });
  }
});
