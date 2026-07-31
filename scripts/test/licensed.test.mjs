/**
 * Which directories need the notice, and what counts as having it.
 *
 * The interesting case is not a missing file — that one announces itself the
 * first time anybody looks in a tarball. It is twenty-eight copies of one
 * paragraph drifting apart, which nothing announces at all, so the reading is
 * byte-for-byte and this is where that is pinned down.
 */

import { describe, expect, it } from "vite-plus/test";

import {
  needsCommittedLicence,
  publishedCrates,
  publishedPackages,
  unlicensed,
} from "../licensed.mjs";

const NOTICE = "MIT License\n\nCopyright (c) 2026 ubugeeei\n";

/** A tree that answers for the files it was given and throws for the rest. */
function tree(files) {
  return (path) => {
    if (!(path in files)) throw new Error(`no such file: ${path}`);
    return files[path];
  };
}

describe("what has to carry the notice", () => {
  it("counts every crate cargo would publish", () => {
    const crates = publishedCrates();

    expect(crates).toContain("crates/slidx_core");
    expect(crates).toContain("crates/slidx_cli");
  });

  it("leaves out a crate that opted out of publishing", () => {
    // `slidx_docs` builds this project's own documentation site and has no
    // reader outside this repository.
    expect(publishedCrates()).not.toContain("crates/slidx_docs");
  });

  it("counts every package npm would publish, including the unscoped wrapper", () => {
    const packages = publishedPackages();

    expect(packages).toContain("packages/vite-plugin");
    expect(packages).toContain("packages/cli");
  });

  it("leaves out a package marked private", () => {
    // A marketplace extension is not an npm package.
    expect(publishedPackages()).not.toContain("packages/vscode");
  });
});

describe("which of them keeps a committed copy", () => {
  it("asks for a copy in a directory whose contents are kept", () => {
    expect(needsCommittedLicence(["packages/one"], () => new Set())).toEqual(["packages/one"]);
  });

  it("leaves out a directory whose copy its build writes", () => {
    // `packages/wasm` is generated (dist, README, and licence alike), so the
    // notice reaches the tarball without ever being in the tree, and
    // `.gitignore` is where that is already said.
    const ignoredCopy = new Set(["packages/wasm/LICENSE"]);

    expect(needsCommittedLicence(["packages/one", "packages/wasm"], () => ignoredCopy)).toEqual([
      "packages/one",
    ]);
  });

  it("reads .gitignore as it stands, so packages/wasm needs no committed copy", () => {
    const directories = needsCommittedLicence(publishedPackages());

    expect(publishedPackages()).toContain("packages/wasm");
    expect(directories).not.toContain("packages/wasm");
    expect(directories).toContain("packages/vite-plugin");
  });
});

describe("reading the notice out of a directory", () => {
  it("says nothing about a directory that carries it exactly", () => {
    const read = tree({ "packages/one/LICENSE": NOTICE });

    expect(unlicensed(["packages/one"], NOTICE, read)).toEqual([]);
  });

  it("reports a directory with no notice at all", () => {
    expect(unlicensed(["packages/one"], NOTICE, tree({}))).toEqual([
      { directory: "packages/one", problem: "missing" },
    ]);
  });

  it("reports a copy that has drifted, which nothing else would catch", () => {
    // A year renumbered in one of twenty-eight copies. Present, readable, and
    // no longer the same statement as the others.
    const read = tree({ "packages/one/LICENSE": NOTICE.replace("2026", "2025") });

    expect(unlicensed(["packages/one"], NOTICE, read)).toEqual([
      { directory: "packages/one", problem: "differs" },
    ]);
  });

  it("reports every directory rather than stopping at the first", () => {
    const read = tree({ "b/LICENSE": "something else" });

    expect(unlicensed(["a", "b", "c"], NOTICE, read).map(({ directory }) => directory)).toEqual([
      "a",
      "b",
      "c",
    ]);
  });
});
