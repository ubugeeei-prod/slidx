/**
 * The installer, actually installing something.
 *
 * A release is served over `file://` from a scratch directory: the same
 * archive, the same `SHA256SUMS`, the same download-verify-extract path, with
 * nothing mocked out. The two cases worth having are the ones a dry run cannot
 * reach — a good download that ends up executable on the PATH, and a bad one
 * that leaves nothing behind.
 *
 * Skipped on Windows, where there is no `sh` to run and `npm i -g slidx` is the
 * install channel anyway.
 */

import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vite-plus/test";

import { assetName, CHECKSUM_FILE, posixPlatforms } from "../../../scripts/platforms.mjs";

const ROOT = join(import.meta.dirname, "../../..");
const INSTALLER = join(ROOT, "install.sh");

/** The platform this test can actually run the installed binary on. */
const HERE = posixPlatforms().find(
  (platform) => platform.os === process.platform && platform.cpu === process.arch,
);

const scratches = [];

afterEach(() => {
  while (scratches.length > 0) rmSync(scratches.pop(), { recursive: true, force: true });
});

function scratch(name) {
  const path = mkdtempSync(join(tmpdir(), `slidx-${name}-`));
  scratches.push(path);
  return path;
}

/**
 * A release directory holding one archive and a checksum file.
 *
 * `corrupt` rewrites the archive after the hashes are taken, which is what a
 * truncated download or a swapped asset looks like from the installer's side.
 */
function publishRelease({ corrupt = false } = {}) {
  const release = scratch("release");
  const stage = scratch("stage");
  const asset = assetName(HERE);

  // Something that runs and prints, so the installer's own report — which
  // shells out to `slidx --version` — has something real to read.
  writeFileSync(join(stage, "slidx"), "#!/bin/sh\necho 'slidx 9.9.9-test'\n", { mode: 0o755 });
  execFileSync("tar", ["-czf", join(release, asset), "-C", stage, "slidx"]);

  const sums = execFileSync("sh", ["-c", `cd '${release}' && shasum -a 256 '${asset}'`], {
    encoding: "utf8",
  });
  writeFileSync(join(release, CHECKSUM_FILE), sums);

  if (corrupt) writeFileSync(join(release, asset), "not the archive that was hashed");

  return release;
}

function install(release, home, expectFailure = false, temp = undefined) {
  const env = {
    ...process.env,
    ...(temp ? { TMPDIR: temp } : {}),
    SLIDX_BASE_URL: `file://${release}`,
    SLIDX_HOME: home,
    SLIDX_OS: process.platform === "darwin" ? "Darwin" : "Linux",
    SLIDX_ARCH:
      process.arch === "arm64" && process.platform === "darwin"
        ? "arm64"
        : process.arch === "arm64"
          ? "aarch64"
          : "x86_64",
  };

  try {
    const stdout = execFileSync("sh", [INSTALLER], { encoding: "utf8", env, stdio: "pipe" });
    if (expectFailure) throw new Error(`the install succeeded:\n${stdout}`);
    return { status: 0, stdout };
  } catch (error) {
    if (!expectFailure) throw new Error(`the install failed:\n${error.stderr ?? error.message}`);
    return { status: error.status, stderr: error.stderr ?? "" };
  }
}

describe.skipIf(process.platform === "win32" || !HERE)("installing from a release", () => {
  it("puts an executable slidx where it said it would", () => {
    const home = scratch("home");
    install(publishRelease(), home);

    const binary = join(home, "bin", "slidx");
    expect(existsSync(binary)).toBe(true);
    expect(execFileSync(binary, { encoding: "utf8" })).toContain("slidx");
  });

  it("says what it installed and where", () => {
    const home = scratch("home");
    const { stdout } = install(publishRelease(), home);

    expect(stdout).toContain("9.9.9-test");
    expect(stdout).toContain(join(home, "bin", "slidx"));
  });

  it("says how to put it on the PATH when it is not there", () => {
    // Which is the state every first install is in. Leaving somebody with a
    // binary they cannot invoke is most of the way to no install at all.
    const home = scratch("home");
    const { stdout } = install(publishRelease(), home);

    expect(stdout).toContain("is not on your PATH");
    expect(stdout).toContain(`export PATH="${join(home, "bin")}:$PATH"`);
  });

  it("installs nothing when the archive does not match the published hash", () => {
    // The whole reason the checksum is not optional. A corrupted or swapped
    // download has to leave the machine exactly as it found it.
    const home = scratch("home");
    const { status, stderr } = install(publishRelease({ corrupt: true }), home, true);

    expect(status).not.toBe(0);
    expect(stderr).toContain("checksum mismatch");
    expect(existsSync(join(home, "bin", "slidx"))).toBe(false);
  });

  it("refuses a release whose checksum file does not mention the asset", () => {
    // An installer pointed at a release built before this platform existed
    // would otherwise verify nothing and report success.
    const release = publishRelease();
    writeFileSync(join(release, CHECKSUM_FILE), "0000  some-other-file.tar.gz\n");

    const home = scratch("home");
    const { stderr } = install(release, home, true);

    expect(stderr).toContain("cannot be verified");
    expect(existsSync(join(home, "bin", "slidx"))).toBe(false);
  });

  it("leaves nothing behind in the temporary directory when it fails", () => {
    // A binary that failed verification must not be sitting in /tmp for
    // somebody to run by hand. Given a TMPDIR of its own so this observes the
    // installer rather than everything else on the machine.
    const temp = scratch("tmp");
    install(publishRelease({ corrupt: true }), scratch("home"), true, temp);

    expect(readdirSync(temp)).toEqual([]);
  });

  it("replaces an older binary rather than failing on the second run", () => {
    // Upgrading is the common case after the first day.
    const release = publishRelease();
    const home = scratch("home");

    install(release, home);
    mkdirSync(join(home, "bin"), { recursive: true });
    writeFileSync(join(home, "bin", "slidx"), "#!/bin/sh\necho old\n", { mode: 0o755 });

    install(release, home);
    expect(readFileSync(join(home, "bin", "slidx"), "utf8")).toContain("9.9.9-test");
  }, 10_000);
});
