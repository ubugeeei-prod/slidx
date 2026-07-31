/**
 * The editor module is read when it is asked for, not once per process.
 *
 * It used to be held for the life of the server, which is right for a file that
 * cannot change and wrong for this one: the people this route exists for are
 * the people editing slidx, and for them it changes on every build. A dev
 * server started before a build served the old editor until it was restarted,
 * and neither rebuilding nor reloading said otherwise.
 *
 * That cost an afternoon of "it is still broken" against a fix that was on disk
 * and correct the whole time.
 */

import { describe, expect, it } from "vite-plus/test";

import { readEditor } from "../src/editor";

describe("serving the editor module", () => {
  it("reads it again rather than answering from memory", async () => {
    // What a build looks like from here: the same path, different bytes.
    let built = "the editor before the build";
    const read = (() =>
      Promise.resolve(built)) as unknown as typeof import("node:fs/promises").readFile;

    expect(await readEditor(read)).toBe("the editor before the build");

    built = "the editor after it";

    expect(await readEditor(read)).toBe("the editor after it");
  });

  it("still resolves the real package by default", async () => {
    // The default path is the one a dev server takes, and a test that only ever
    // injected a reader would not notice it resolving nothing.
    await expect(readEditor()).resolves.toContain("slidx");
  });
});
