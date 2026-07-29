/**
 * The npm channel, actually running a binary.
 *
 * A `node_modules` is laid out the way npm lays one out — the `slidx` wrapper
 * next to the one platform package that matches this machine — and the shim is
 * run from inside it. That exercises the part the unit tests cannot: whether
 * `require.resolve` finds the binary through a real directory layout, and
 * whether the exit code and the streams come back through the extra process.
 *
 * Skipped on Windows, where the stand-in binary would have to be a real
 * executable rather than a shell script.
 */

import { execFileSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import { binaryName, PLATFORMS } from "../../../scripts/platforms.mjs";

const ROOT = join(import.meta.dirname, "../../..");

/** The platform package npm would install on the machine running this test. */
const HERE = PLATFORMS.find(
  (platform) => platform.os === process.platform && platform.cpu === process.arch,
);

const scratches = [];

afterEach(() => {
  while (scratches.length > 0) rmSync(scratches.pop(), { recursive: true, force: true });
});

/**
 * A node_modules holding the wrapper and, optionally, its platform package.
 *
 * Built by running the real generator over a stand-in binary, so the layout
 * under test is the one a release produces rather than one this file invented.
 */
function installTree({ withPlatformPackage = true } = {}) {
  const root = mkdtempSync(join(tmpdir(), "slidx-npm-"));
  scratches.push(root);

  const wrapper = join(root, "node_modules", "slidx");
  mkdirSync(join(wrapper, "bin"), { recursive: true });
  copyFileSync(join(ROOT, "packages/cli/package.json"), join(wrapper, "package.json"));
  copyFileSync(join(ROOT, "packages/cli/bin/slidx.mjs"), join(wrapper, "bin", "slidx.mjs"));

  if (withPlatformPackage) {
    // Every target, because that is what a release produces and what the
    // generator insists on — then only the matching one is installed, which is
    // what npm does with optional dependencies.
    for (const platform of PLATFORMS) {
      const staged = join(root, "staged", platform.target);
      mkdirSync(staged, { recursive: true });
      writeFileSync(
        join(staged, binaryName(platform)),
        '#!/bin/sh\nif [ "$1" = fail ]; then exit 1; fi\necho "slidx 9.9.9-test $*"\n',
        { mode: 0o755 },
      );
    }

    execFileSync(
      "node",
      ["scripts/build-platform-packages.mjs", join(root, "staged"), join(root, "generated")],
      { cwd: ROOT, stdio: "pipe" },
    );

    const name = HERE.npm.replace("@slidx/", "");
    mkdirSync(join(root, "node_modules", "@slidx"), { recursive: true });
    execFileSync("cp", ["-R", join(root, "generated", name), join(root, "node_modules", HERE.npm)]);
  }

  return join(wrapper, "bin", "slidx.mjs");
}

function run(shim, args = []) {
  try {
    return { status: 0, stdout: execFileSync("node", [shim, ...args], { encoding: "utf8" }) };
  } catch (error) {
    return { status: error.status, stdout: error.stdout ?? "", stderr: error.stderr ?? "" };
  }
}

describe.skipIf(process.platform === "win32" || !HERE)("npm i -g slidx", () => {
  it("runs the binary from the platform package npm installed", () => {
    const { status, stdout } = run(installTree(), ["doctor"]);

    expect(status).toBe(0);
    expect(stdout).toContain("slidx 9.9.9-test");
  });

  it("passes its arguments through untouched", () => {
    // A wrapper that ate a flag would make `slidx lint --allow contrast` mean
    // something different from the binary's own behaviour.
    const { stdout } = run(installTree(), ["lint", "slides", "--allow", "contrast"]);

    expect(stdout).toContain("lint slides --allow contrast");
  });

  it("returns the binary's exit code, so CI still fails on a blocking finding", () => {
    // The exit code is the reason `slidx lint` exists. A shim that swallowed
    // it would turn every CI run green.
    expect(run(installTree(), ["fail"]).status).toBe(1);
  });

  it("ships the binary executable, which npm restores on install", () => {
    const shim = installTree();
    const binary = join(shim, "..", "..", "..", HERE.npm, "bin", binaryName(HERE));

    expect(statSync(binary).mode & 0o111).not.toBe(0);
  });

  it("runs when it is reached through a symlink, which is how pnpm installs", () => {
    // Node reports the entry point as its real path while process.argv[1]
    // keeps the symlinks used to reach it. Compared as strings, the shim does
    // nothing at all and exits 0 — a silent no-op on every pnpm machine.
    const shim = installTree();
    const link = join(shim, "..", "..", "..", ".bin-slidx");
    symlinkSync(shim, link);

    expect(run(link, ["doctor"]).stdout).toContain("slidx 9.9.9-test");
  });

  it("exits 2 with an explanation when the platform package is missing", () => {
    // `--omit=optional`, or a lockfile from another platform. Distinguishable
    // from a finding, and it says which of the usual causes to look at.
    const { status, stderr } = run(installTree({ withPlatformPackage: false }), ["doctor"]);

    expect(status).toBe(2);
    expect(stderr).toContain("--no-optional");
  });
});
