/**
 * What a build does on a machine that has no browser.
 *
 * Which is most machines: `pnpm install` does not download browsers, so every
 * first checkout and every CI runner that is not Linux is in this state. A
 * social card is a nicety and a deck is not, so the deck has to come out.
 *
 * The intent was already written — both callers warn and carry on when the
 * measurement returns nothing — and it could not be reached. `rasterise` and
 * `measureOverflow` caught a missing *package* and not a missing *browser
 * binary*, and those are different throws from different lines.
 */

import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterAll, beforeAll, describe, expect, it } from "vite-plus/test";

import { rasterise } from "../src/og";
import { measureOverflow } from "../src/overflow";

const CARD = '<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630"></svg>';

let restore: string | undefined;

beforeAll(() => {
  restore = process.env["PLAYWRIGHT_BROWSERS_PATH"];
  // An empty directory is a machine with the package installed and no browser
  // in it, which is the state the caught error was never about.
  process.env["PLAYWRIGHT_BROWSERS_PATH"] = mkdtempSync(join(tmpdir(), "slidx-no-browsers-"));
});

afterAll(() => {
  if (restore === undefined) delete process.env["PLAYWRIGHT_BROWSERS_PATH"];
  else process.env["PLAYWRIGHT_BROWSERS_PATH"] = restore;
});

describe("with no browser to launch", () => {
  it("answers nothing for a social card rather than failing the build", async () => {
    await expect(rasterise(CARD)).resolves.toBeNull();
  });

  it("answers nothing for the overflow measurement, which is unchecked and not clean", async () => {
    // The distinction the linter makes everywhere else: a rule that could not
    // run reports that it could not run. Returning an empty measurement here
    // would report a deck as fitting when nothing looked.
    await expect(measureOverflow("does-not-matter.html")).resolves.toBeNull();
  });
});
