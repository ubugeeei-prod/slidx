/**
 * The shell installer, checked as data rather than by installing anything.
 *
 * `install.sh --dry-run` resolves a platform, names the asset and the
 * destination, and downloads nothing. That is a useful thing for a person to
 * be able to ask, and it is what makes the interesting cases — an architecture
 * this machine is not, a platform nobody publishes — reachable from a test
 * instead of from six laptops.
 *
 * Written in JavaScript rather than TypeScript because it reads
 * `scripts/platforms.mjs`, which the build scripts also read; the alternative
 * is a second copy of the table with a `.ts` extension, which is the exact
 * thing that table exists to prevent.
 */

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { assetName, CHECKSUM_FILE, posixPlatforms } from "../../../scripts/platforms.mjs";

// `import.meta.dirname` rather than `new URL(..., import.meta.url)`: under
// Vite's module runner `import.meta.url` is not a file URL.
const INSTALLER = join(import.meta.dirname, "../../../install.sh");
const SOURCE = readFileSync(INSTALLER, "utf8");

/**
 * What the script actually runs, with everything it merely says taken out.
 *
 * Comments and the help text are prose: the sentence promising no sudo must
 * not fail the assertion that enforces it.
 */
const STATEMENTS = SOURCE.replace(/<<'USAGE'[\s\S]*?\nUSAGE\n/, "")
  .split("\n")
  .filter((line) => !line.trimStart().startsWith("#"))
  .join("\n");

/**
 * Runs the installer with detection overridden, and nothing downloaded.
 *
 * The knobs this test suite is about are read from the environment, so the
 * machine running the tests has to be scrubbed out of it — otherwise a
 * developer with XDG_DATA_HOME set gets a different answer from CI.
 */
function dryRun(os, arch, extra = {}) {
  const env = { ...process.env, SLIDX_OS: os, SLIDX_ARCH: arch, HOME: "/home/somebody" };
  for (const knob of ["XDG_DATA_HOME", "SLIDX_HOME", "SLIDX_VERSION"]) delete env[knob];

  return execFileSync("sh", [INSTALLER, "--dry-run"], {
    encoding: "utf8",
    env: { ...env, ...extra },
  });
}

function refuse(os, arch) {
  try {
    dryRun(os, arch);
  } catch (error) {
    return { status: error.status, stderr: error.stderr };
  }

  throw new Error(`${os} ${arch} was accepted, and there is no binary for it`);
}

/** How `uname -s` and `uname -m` spell each published platform. */
const UNAME = {
  "aarch64-apple-darwin": ["Darwin", "arm64"],
  "x86_64-apple-darwin": ["Darwin", "x86_64"],
  "x86_64-unknown-linux-musl": ["Linux", "x86_64"],
  "aarch64-unknown-linux-musl": ["Linux", "aarch64"],
};

describe("platform detection", () => {
  it("resolves every platform the release publishes a binary for", () => {
    // The drift that matters most: a platform the workflow builds and the
    // installer cannot name is a download nobody can reach.
    for (const platform of posixPlatforms()) {
      const [os, arch] = UNAME[platform.target];
      expect(dryRun(os, arch)).toContain(platform.target);
    }
  });

  it("names the asset the release actually uploads", () => {
    for (const platform of posixPlatforms()) {
      const [os, arch] = UNAME[platform.target];
      expect(dryRun(os, arch)).toContain(assetName(platform));
    }
  });

  it("accepts the other spellings people's machines use for the same chip", () => {
    // A shell that says amd64, or arm64 on Linux where the triple says
    // aarch64. Both are the same machine and both have to resolve.
    expect(dryRun("Linux", "amd64")).toContain("x86_64-unknown-linux-musl");
    expect(dryRun("Linux", "arm64")).toContain("aarch64-unknown-linux-musl");
  });

  it("refuses a platform it has no binary for instead of installing something that will not run", () => {
    // Finding out at a lectern that the binary is for another architecture is
    // worse than finding out now that there is not one.
    const { status, stderr } = refuse("Linux", "riscv64");

    expect(status).not.toBe(0);
    expect(stderr).toContain("no prebuilt binary");
  });

  it("tells an unsupported platform what is published and how to build from source", () => {
    const { stderr } = refuse("SunOS", "sparc");

    for (const platform of posixPlatforms()) {
      expect(stderr).toContain(platform.target);
    }
    expect(stderr).toContain("cargo install");
  });

  it("sends Windows to npm rather than leaving it at a dead end", () => {
    // There is a Windows binary; this script is not how you get it.
    expect(refuse("MINGW64_NT-10.0", "x86_64").stderr).toContain("npm i -g slidx");
  });
});

describe("what it reports before doing anything", () => {
  it("says where the binary will go", () => {
    expect(dryRun("Linux", "x86_64")).toContain("/home/somebody/.slidx/bin/slidx");
  });

  it("puts the binary where the version manager will look, not in a second place", () => {
    // `curl | sh` and `slidx version use` have to agree on one directory, or
    // one of them is silently not managing the binary that runs.
    expect(dryRun("Linux", "x86_64", { XDG_DATA_HOME: "/home/somebody/.local/share" })).toContain(
      "/home/somebody/.local/share/slidx/bin/slidx",
    );
  });

  it("honours an explicit install root", () => {
    expect(dryRun("Linux", "x86_64", { SLIDX_HOME: "/opt/slidx" })).toContain(
      "/opt/slidx/bin/slidx",
    );
  });

  it("installs the latest release unless a version is asked for", () => {
    expect(dryRun("Linux", "x86_64")).toContain("latest");
    expect(dryRun("Linux", "x86_64", { SLIDX_VERSION: "v9.9.9" })).toContain("v9.9.9");
  });

  it("says the download will be verified", () => {
    expect(dryRun("Linux", "x86_64")).toContain(CHECKSUM_FILE);
  });

  it("downloads nothing on a dry run", () => {
    expect(dryRun("Linux", "x86_64")).toContain("Nothing was downloaded");
  });
});

describe("the promises the script makes about itself", () => {
  it("calls main on the very last line so a truncated download does nothing", () => {
    // `curl | sh` executes the script as it arrives. Written as a list of
    // statements, a dropped connection runs half an install.
    const lines = SOURCE.trimEnd().split("\n");

    expect(lines[lines.length - 1]).toBe('main "$@"');
  });

  it("never asks for root", () => {
    // A script people are asked to pipe into a shell must not also want root.
    // Checked against what it runs rather than what it says, so the comment
    // promising no sudo does not fail the test that enforces it.
    expect(STATEMENTS).not.toMatch(/\bsudo\b/);
    expect(STATEMENTS).not.toMatch(/\bdoas\b/);
  });

  it("writes everything it installs under a directory the user owns", () => {
    // The other half of not needing root: nothing goes to /usr/local, where
    // an unprivileged install would fail and a privileged one would spread
    // root-owned files through somebody's machine.
    expect(STATEMENTS).not.toContain("/usr/local");
    expect(STATEMENTS).not.toContain("/opt/");
    expect(dryRun("Linux", "x86_64")).toContain("/home/somebody/.slidx");
  });

  it("refuses to continue without a way to compute a checksum", () => {
    // The failure mode this rules out: an installer that shrugs and installs
    // anyway on a machine with no sha256sum.
    expect(SOURCE).toContain("that is not optional");
  });

  it("treats an asset missing from the checksum file as a failure", () => {
    // What an installer pointed at a release built before this platform
    // existed would otherwise do: verify nothing and report success.
    expect(SOURCE).toContain("cannot be verified");
  });

  it("cleans up whatever it downloaded however it exits", () => {
    // A binary that failed verification must not be left in /tmp where
    // somebody can run it by hand.
    expect(SOURCE).toMatch(/trap 'rm -rf "\$tmp"' EXIT/);
  });

  it("is POSIX sh rather than bash, because that is what the pipe lands in", () => {
    expect(SOURCE.startsWith("#!/bin/sh\n")).toBe(true);
  });

  it("prints its help without reading its own source, which a pipe does not have", () => {
    // `curl | sh` leaves $0 as `sh` and the script on standard input. Anything
    // that reads its own file prints the wrong thing — and that pipe is how
    // most people will ever run this.
    const piped = execFileSync("sh", ["-c", `cat ${INSTALLER} | sh -s -- --help`], {
      encoding: "utf8",
    });

    expect(piped).toContain("slidx installer");
    expect(piped).toContain("--dry-run");
    expect(piped).toContain("SLIDX_HOME");
  });

  it("stops on an unset variable rather than deleting a path that expanded to nothing", () => {
    expect(SOURCE).toContain("set -eu");
  });

  it("points at the build attestation for the threat a checksum does not cover", () => {
    // Being straight about it: the hash and the binary come from the same
    // server, so it proves the download arrived intact and unswapped, not that
    // the account was not compromised.
    expect(SOURCE).toContain("gh attestation verify");
  });
});
