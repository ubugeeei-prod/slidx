/**
 * The README's first impression, kept on the assets and tokens this repository
 * actually ships.
 *
 * GitHub's shell grammar treats `cd` and `export` as commands wherever they
 * occur, including as slidx subcommands. The CLI examples are transcripts, not
 * shell programs, so a plain-text fence keeps the product's own words visually
 * equal instead of colouring two of them as if the shell would execute them.
 */

import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vite-plus/test";

const ROOT = join(import.meta.dirname, "../..");
const README = readFileSync(join(ROOT, "README.md"), "utf8");

describe("the README identity", () => {
  it("uses the generated lockup in both colour schemes", () => {
    for (const asset of ["assets/brand/lockup-light.svg", "assets/brand/lockup-dark.svg"]) {
      expect(README, asset).toContain(`./${asset}`);
      expect(existsSync(join(ROOT, asset)), asset).toBe(true);
    }
  });

  it("links animated tour previews to their full recordings", () => {
    for (const name of ["editor-tour", "cli-tour"]) {
      const preview = `./docs/media/${name}.png`;
      const video = `./docs/media/${name}.webm`;

      expect(README, name).toContain(`<a href="${video}">`);
      expect(README, name).toContain(`src="${preview}"`);
      expect(existsSync(join(ROOT, preview.slice(2))), preview).toBe(true);
      expect(existsSync(join(ROOT, video.slice(2))), video).toBe(true);
    }
  });
});

// The CLI's own section, which is the part written as transcripts; the fences
// after it belong to the export and benchmark sections.
const CLI = README.slice(README.indexOf("## The CLI"), README.indexOf("## Give the talk"));

describe("the CLI examples", () => {
  it("do not ask a shell grammar to colour subcommands", () => {
    expect(CLI).toContain("slidx export --target pdf");
    expect(CLI).toContain("slidx cd vueconf");
    expect(CLI).not.toContain("```bash");
    expect(CLI.match(/```text/g)).toHaveLength(3);
  });

  it("introduce commands in the order a talk needs them", () => {
    const fences = [...CLI.matchAll(/```text\n([\s\S]*?)```/g)].map((fence) =>
      fence[1]
        .trim()
        .split("\n")
        .map((line) => line.match(/^slidx ([^ ]+)/)?.[1]),
    );

    // Making a deck, then giving the talk, then finding the deck you forgot.
    expect(fences).toEqual([
      ["create", "add", "save"],
      ["dev", "fmt", "lint", "export", "doctor", "publish"],
      ["list", "grep", "open", "cd", "mv", "rm"],
    ]);
  });
});
