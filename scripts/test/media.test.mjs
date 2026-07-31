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
const EDITOR_RECORDER = readFileSync(join(ROOT, "scripts/record-editor-tour.mjs"), "utf8");
const MEDIA_RECORDER = readFileSync(join(ROOT, "scripts/record.mjs"), "utf8");

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

  it("records tours in dark mode with eased, frame-visible gestures", () => {
    expect(EDITOR_RECORDER).toContain('const TOUR_SCHEME = "dark"');
    expect(EDITOR_RECORDER).toContain("const POINTER_STEPS = 28");
    expect(EDITOR_RECORDER).toContain("easeInOut(step / POINTER_STEPS)");
    expect(EDITOR_RECORDER).toContain(".slidx-freeform-color-input");
    expect(EDITOR_RECORDER).toContain('data-handle="se"');
    expect(EDITOR_RECORDER).toContain(".slidx-freeform-move");
    expect(EDITOR_RECORDER).toContain("new File([bytes], item.name");
    expect(EDITOR_RECORDER).toContain('video[src*="tour-motion.webm"]');
    expect(EDITOR_RECORDER).toContain("SLIDX_SHARE_EDIT");
    expect(EDITOR_RECORDER).toContain("resolvedUrls.network");
    expect(EDITOR_RECORDER).not.toContain('colorScheme: "light"');

    expect(MEDIA_RECORDER).toContain('const TOUR_SCHEME = "dark"');
    expect(MEDIA_RECORDER).toContain("const TYPE_DELAY_MS = 42");
    expect(MEDIA_RECORDER).toContain("tour-output-in 240ms");
    expect(MEDIA_RECORDER).toContain("portableTourOutput");
    expect(MEDIA_RECORDER).toContain('replaceAll(realpathSync(directory), "~/slides")');
  });
});
