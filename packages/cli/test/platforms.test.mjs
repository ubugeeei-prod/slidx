/**
 * The platform list, checked wherever it is repeated.
 *
 * `scripts/platforms.mjs` is the table. The release workflow cannot import it —
 * YAML has no imports — and neither can the shell installer, so those two carry
 * copies. A copy that falls out of step is a release that builds a binary
 * nobody can download, or publishes a package nobody can install, and neither
 * shows up until somebody tries it on the platform that was dropped.
 *
 * These tests are that copy's only defence.
 */

import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterEach, describe, expect, it } from "vite-plus/test";

import {
  assetName,
  binaryName,
  CHECKSUM_FILE,
  PLATFORMS,
  posixPlatforms,
} from "../../../scripts/platforms.mjs";

const ROOT = join(import.meta.dirname, "../../..");
const WORKFLOW = readFileSync(join(ROOT, ".github/workflows/release.yml"), "utf8");
const SETUP = readFileSync(join(ROOT, ".github/actions/setup/action.yml"), "utf8");
const INSTALLER = readFileSync(join(ROOT, "install.sh"), "utf8");
const WRAPPER = JSON.parse(readFileSync(join(ROOT, "packages/cli/package.json"), "utf8"));

const scratches = [];

afterEach(() => {
  while (scratches.length > 0) rmSync(scratches.pop(), { recursive: true, force: true });
});

function scratch() {
  const path = mkdtempSync(join(tmpdir(), "slidx-platforms-"));
  scratches.push(path);
  return path;
}

/** Lays out one binary per target, as the release workflow's artifacts do. */
function withBinaries(platforms) {
  const directory = scratch();

  for (const platform of platforms) {
    mkdirSync(join(directory, platform.target), { recursive: true });
    writeFileSync(join(directory, platform.target, binaryName(platform)), "#!/bin/sh\n");
  }

  return directory;
}

function generate(binaries, outDir) {
  return execFileSync("node", ["scripts/build-platform-packages.mjs", binaries, outDir], {
    cwd: ROOT,
    encoding: "utf8",
  });
}

describe("the table and its copies", () => {
  it("names every platform in the release matrix", () => {
    // A target the workflow does not build is an asset the installer will 404
    // on and a package the wrapper will fail to resolve.
    for (const platform of PLATFORMS) {
      expect(WORKFLOW).toContain(platform.target);
      expect(WORKFLOW).toContain(platform.runner);
    }
  });

  it("builds nothing the table does not declare", () => {
    // The other direction: a target left in the matrix after being dropped
    // from the table uploads an asset nothing knows how to install.
    const targets = [...WORKFLOW.matchAll(/target: (\S+)/g)].map((match) => match[1]);

    expect(targets.length).toBeGreaterThan(0);
    for (const target of targets) {
      expect(PLATFORMS.map((platform) => platform.target)).toContain(target);
    }
  });

  it("is spelled the same way in the shell installer", () => {
    for (const platform of posixPlatforms()) {
      expect(INSTALLER).toContain(platform.target);
    }
  });

  it("leaves Windows out of the shell installer on purpose", () => {
    // There is a Windows binary. `curl | sh` is not how anyone gets it, and an
    // installer that listed it would be offering something it cannot deliver.
    for (const platform of PLATFORMS.filter((entry) => entry.windows)) {
      expect(INSTALLER).not.toContain(platform.target);
    }
  });

  it("is the same list the npm wrapper depends on", () => {
    expect(Object.keys(WRAPPER.optionalDependencies).sort()).toEqual(
      PLATFORMS.map((platform) => platform.npm).sort(),
    );
  });

  it("agrees with the wrapper about the version, so a release cannot pair a new shim with an old binary", () => {
    for (const version of Object.values(WRAPPER.optionalDependencies)) {
      expect(version).toBe(WRAPPER.version);
    }
  });

  it("keeps the wrapper out of the pnpm workspace, so the repository can be installed", () => {
    // Its only dependencies are the five platform packages, which exist on the
    // registry and nowhere else. In the workspace, `pnpm install
    // --frozen-lockfile` fails on a package that has never been published —
    // and it cannot be published from a repository that will not install.
    const workspace = readFileSync(join(ROOT, "pnpm-workspace.yaml"), "utf8");

    expect(workspace).toContain('"!packages/cli"');
  });

  it("names the checksum file the same way in the installer and the release", () => {
    expect(INSTALLER).toContain(CHECKSUM_FILE);
    expect(WORKFLOW).toContain(CHECKSUM_FILE);
  });

  it("uploads the asset name the installer asks for", () => {
    // Both are built from the target triple, so this checks the shape rather
    // than five literal strings.
    for (const platform of posixPlatforms()) {
      expect(assetName(platform)).toBe(`slidx-${platform.target}.tar.gz`);
    }
    expect(WORKFLOW).toContain("slidx-${{ matrix.target }}");
  });

  it("publishes through npm's supported OIDC runner without disabling Blacksmith caches", () => {
    const npmJob = WORKFLOW.slice(
      WORKFLOW.indexOf("\n  npm:"),
      WORKFLOW.indexOf("\n  github-release:"),
    );

    expect(npmJob).toContain("runs-on: ubuntu-latest");
    expect(npmJob).toContain("id-token: write");
    expect(npmJob).toContain("uses: actions/checkout@v6");
    expect(npmJob).toContain('sticky: "false"');
    expect(npmJob).not.toContain("useblacksmith/");

    expect(SETUP).toContain("sticky:");
    expect(SETUP).toContain('default: "true"');
    expect(SETUP).toContain("uses: useblacksmith/stickydisk@v1");
  });
});

describe("building the platform packages", () => {
  it("produces one package per platform, each holding its own binary", () => {
    const out = scratch();
    generate(withBinaries(PLATFORMS), out);

    for (const platform of PLATFORMS) {
      const directory = join(out, platform.npm.replace("@slidxjs/", ""));
      expect(existsSync(join(directory, "bin", binaryName(platform)))).toBe(true);
    }
  });

  it("declares the os and cpu npm filters on, so only one is ever installed", () => {
    const out = scratch();
    generate(withBinaries(PLATFORMS), out);

    for (const platform of PLATFORMS) {
      const manifest = JSON.parse(
        readFileSync(join(out, platform.npm.replace("@slidxjs/", ""), "package.json"), "utf8"),
      );

      expect(manifest.name).toBe(platform.npm);
      expect(manifest.os).toEqual([platform.os]);
      expect(manifest.cpu).toEqual([platform.cpu]);
    }
  });

  it("takes its version from the Cargo workspace rather than repeating it", () => {
    const out = scratch();
    generate(withBinaries(PLATFORMS), out);

    const manifest = JSON.parse(readFileSync(join(out, "cli-linux-x64/package.json"), "utf8"));
    expect(manifest.version).toBe(WRAPPER.version);
  });

  it("declares a platform-specific bin that cannot shadow the wrapper on PATH", () => {
    const out = scratch();
    generate(withBinaries(PLATFORMS), out);

    const manifest = JSON.parse(readFileSync(join(out, "cli-darwin-arm64/package.json"), "utf8"));
    expect(manifest.bin).toEqual({ "slidx-darwin-arm64": "bin/slidx" });
    expect(manifest.bin.slidx).toBeUndefined();
  });

  it.skipIf(process.platform === "win32")(
    "packs the platform binary with an executable mode",
    () => {
      const generated = scratch();
      generate(withBinaries(PLATFORMS), generated);

      const destination = scratch();
      const report = JSON.parse(
        execFileSync(
          "pnpm",
          [
            "--dir",
            join(generated, "cli-darwin-arm64"),
            "pack",
            "--pack-destination",
            destination,
            "--json",
          ],
          { encoding: "utf8" },
        ),
      );
      const filename = (Array.isArray(report) ? report[0] : report).filename;
      const extracted = scratch();

      execFileSync("tar", ["-xzf", resolve(filename), "-C", extracted]);

      expect(statSync(join(extracted, "package/bin/slidx")).mode & 0o111).not.toBe(0);
    },
  );

  it("writes no install script into anything it generates", () => {
    // The point of the whole pattern. A postinstall here would undo it.
    const out = scratch();
    generate(withBinaries(PLATFORMS), out);

    for (const platform of PLATFORMS) {
      const manifest = JSON.parse(
        readFileSync(join(out, platform.npm.replace("@slidxjs/", ""), "package.json"), "utf8"),
      );
      expect(manifest.scripts).toBeUndefined();
    }
  });

  it("carries the licence into every package rather than leaving it implied", () => {
    const out = scratch();
    generate(withBinaries(PLATFORMS), out);

    expect(existsSync(join(out, "cli-linux-x64/LICENSE"))).toBe(true);
  });

  it("fails rather than publishing a wrapper whose binary is missing for a platform", () => {
    // An empty platform package installs cleanly and fails at the first
    // `slidx`, which is the worst possible moment to discover it.
    const partial = withBinaries(PLATFORMS.slice(0, 2));

    expect(() => generate(partial, scratch())).toThrow();
  });
});
