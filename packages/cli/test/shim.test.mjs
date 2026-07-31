/**
 * The npm shim: how `npm i -g slidx` ends up running a binary.
 *
 * The interesting cases here are all failures — a platform nobody publishes,
 * an install that skipped optional dependencies, a package present but empty —
 * because the happy path is one `spawnSync` and the failures are where somebody
 * is left stuck.
 */

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vite-plus/test";

import { PLATFORMS } from "../../../scripts/platforms.mjs";
import {
  findBinary,
  missingBinaryMessage,
  platformPackage,
  publishedPlatforms,
} from "../bin/slidx.mjs";

const MANIFEST = JSON.parse(readFileSync(join(import.meta.dirname, "../package.json"), "utf8"));
const SHIM = join(import.meta.dirname, "../bin/slidx.mjs");

describe("finding the binary npm installed", () => {
  it("derives the package name from the platform rather than a table of its own", () => {
    // The name is the platform. A lookup table here could only ever disagree
    // with the optionalDependencies next to it.
    expect(platformPackage("darwin", "arm64")).toBe("@slidxjs/cli-darwin-arm64");
    expect(platformPackage("linux", "x64")).toBe("@slidxjs/cli-linux-x64");
    expect(platformPackage("win32", "x64")).toBe("@slidxjs/cli-win32-x64");
  });

  it("names a package for every platform the release builds", () => {
    for (const platform of PLATFORMS) {
      expect(platformPackage(platform.os, platform.cpu)).toBe(platform.npm);
    }
  });

  it("reads what is published from its own manifest rather than a second list", () => {
    expect(publishedPlatforms().sort()).toEqual(PLATFORMS.map((entry) => entry.npm).sort());
  });

  it("returns nothing when the platform package is not installed", () => {
    // Which is the state of this repository: the platform packages exist only
    // after a release builds them.
    expect(findBinary("@slidxjs/cli-nowhere-nothing")).toBeUndefined();
  });
});

describe("what it says when there is no binary", () => {
  it("tells an unpublished platform what is published and how to build from source", () => {
    const message = missingBinaryMessage("@slidxjs/cli-freebsd-x64", publishedPlatforms());

    expect(message).toContain("darwin-arm64");
    expect(message).toContain("linux-x64");
    expect(message).toContain("cargo install slidx_cli");
  });

  it("tells a published platform that its package is missing, and why that happens", () => {
    // A different situation with a different fix, so it gets a different
    // message. "No binary found" would leave somebody reinstalling forever.
    const message = missingBinaryMessage("@slidxjs/cli-linux-x64", publishedPlatforms());

    expect(message).toContain("--no-optional");
    expect(message).toContain("lockfile was copied");
    expect(message).toContain("npm i -g slidx");
  });

  it("offers the shell installer as the way out that does not involve npm", () => {
    const message = missingBinaryMessage("@slidxjs/cli-linux-x64", publishedPlatforms());

    expect(message).toContain("install.sh");
  });

  it("exits 2 rather than 1, because nothing was checked", () => {
    // 1 means slidx ran and found something. A shim that could not start the
    // binary has to be distinguishable from a deck with a problem.
    let status;
    try {
      execFileSync("node", [SHIM, "doctor"], { encoding: "utf8", stdio: "pipe" });
    } catch (error) {
      status = error.status;
    }

    expect(status).toBe(2);
  });
});

describe("the promises the package makes", () => {
  it("has no install scripts of any kind", () => {
    // The whole reason for the platform-package pattern. A postinstall that
    // downloads a binary breaks offline installs, breaks cached CI, breaks
    // behind a proxy, and cannot be audited from the published tarball.
    for (const hook of ["preinstall", "install", "postinstall", "prepare"]) {
      expect(MANIFEST.scripts?.[hook]).toBeUndefined();
    }
  });

  it("writes down why there is no postinstall, where the next person will look", () => {
    const source = readFileSync(SHIM, "utf8");

    expect(source).toContain("Why there is no postinstall script");
    expect(source).toContain("offline");
    expect(source).toContain("supply-chain");
  });

  it("has no runtime dependencies at all", () => {
    // Everything except the binary itself. A wrapper whose job is to exec one
    // process should not pull a tree in to do it.
    expect(MANIFEST.dependencies).toBeUndefined();
  });

  it("declares every platform package as optional rather than required", () => {
    // Required, npm would fail the whole install on any platform but one.
    expect(Object.keys(MANIFEST.optionalDependencies)).toHaveLength(PLATFORMS.length);
    for (const platform of PLATFORMS) {
      expect(MANIFEST.optionalDependencies[platform.npm]).toBe(MANIFEST.version);
    }
  });

  it("pins the platform packages to an exact version rather than a range", () => {
    // A range would let npm pair a new wrapper with an old binary, which is a
    // mismatch nothing else would catch.
    for (const version of Object.values(MANIFEST.optionalDependencies)) {
      expect(version).toMatch(/^\d+\.\d+\.\d+$/);
    }
  });

  it("ships only the shim", () => {
    expect(MANIFEST.files).toEqual(["bin"]);
  });

  it("installs the binary as `slidx`", () => {
    expect(MANIFEST.bin).toEqual({ slidx: "bin/slidx.mjs" });
  });
});
