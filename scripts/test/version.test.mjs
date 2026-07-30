/**
 * A release version is also the version the CLI wrapper asks npm to install.
 * Platform packages are generated in Actions and therefore have no source
 * package.json for a workspace-only check to discover.
 */

import { execFileSync, spawnSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vite-plus/test";

const CHECK = join(import.meta.dirname, "..", "check-version.mjs");
const RELEASE = join(import.meta.dirname, "..", "release.mjs");

describe("the release version check", () => {
  it("rejects an old generated-platform requirement in the CLI wrapper", async () => {
    const root = await fixture("0.1.0");
    const result = spawnSync(process.execPath, [CHECK], { cwd: root, encoding: "utf8" });

    expect(result.status).toBe(1);
    expect(result.stderr).toContain(
      "requires @ubugeeei/slidx-cli-linux-x64 at 0.1.0, but the package release is 0.2.0",
    );
  });

  it("accepts exact generated requirements and workspace-relative ones", async () => {
    const root = await fixture("0.2.0");
    const output = execFileSync(process.execPath, [CHECK], { cwd: root, encoding: "utf8" });

    expect(output).toContain("2 internal package requirement(s)");
  });

  it("rejects a public package name that does not match its workspace directory", async () => {
    const root = await fixture("0.2.0", "@somebody/slidx-plugin");
    const result = spawnSync(process.execPath, [CHECK], { cwd: root, encoding: "utf8" });

    expect(result.status).toBe(1);
    expect(result.stderr).toContain(
      "packages/plugin/package.json: package name must be @ubugeeei/slidx-plugin",
    );
  });

  it("regenerates versioned brand assets before it verifies the release", async () => {
    const source = await readFile(RELEASE, "utf8");
    const generate = source.indexOf('run("pnpm", ["exec", "vp", "run", "generate:brand"]);');
    const verify = source.indexOf('run("node", ["scripts/check-version.mjs", tag]);');

    expect(generate).toBeGreaterThan(-1);
    expect(verify).toBeGreaterThan(generate);
  });
});

async function fixture(platformVersion, pluginName = "@ubugeeei/slidx-plugin") {
  const root = await mkdtemp(join(tmpdir(), "slidx-version-"));
  await mkdir(join(root, "packages", "cli"), { recursive: true });
  await mkdir(join(root, "packages", "plugin"), { recursive: true });
  await writeFile(
    join(root, "Cargo.toml"),
    `[workspace.package]\nversion = "0.2.0"\n\n` +
      `[workspace.dependencies]\nslidx_core = { version = "0.2.0", path = "crates/core" }\n`,
  );
  await writeFile(
    join(root, "Cargo.lock"),
    `version = 4\n\n[[package]]\nname = "slidx_core"\nversion = "0.2.0"\n`,
  );
  await writeFile(
    join(root, "packages", "cli", "package.json"),
    JSON.stringify({
      name: "slidx",
      version: "0.2.0",
      optionalDependencies: { "@ubugeeei/slidx-cli-linux-x64": platformVersion },
    }),
  );
  await writeFile(
    join(root, "packages", "plugin", "package.json"),
    JSON.stringify({
      name: pluginName,
      version: "0.2.0",
      dependencies: { "@ubugeeei/slidx-runtime": "workspace:*" },
    }),
  );
  execFileSync("git", ["init", "-q", "--initial-branch=main"], { cwd: root });
  execFileSync("git", ["add", "."], { cwd: root });
  return root;
}
