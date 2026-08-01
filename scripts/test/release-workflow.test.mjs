import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFileSync } from "node:child_process";
import test from "node:test";

import { assertPublishableManifest, packedManifest } from "../pack-npm.mjs";

const workflow = readFileSync(".github/workflows/release.yml", "utf8");
const guide = readFileSync("RELEASING.md", "utf8");

test("the npm release builds the package graph from the repository root", () => {
  assert.match(workflow, /with:\n\s+wasm: "true"/);
  assert.match(workflow, /run: pnpm exec vp run build:packages/);
  assert.doesNotMatch(workflow, /pnpm -r .* run pack:lib/);
});

test("the npm release resolves workspace dependencies before publishing", async (context) => {
  const destination = await mkdtemp(join(tmpdir(), "slidx-npm-pack-"));
  context.after(() => rm(destination, { recursive: true, force: true }));

  const output = execFileSync(
    process.execPath,
    ["scripts/pack-npm.mjs", destination, "packages/vite-plugin"],
    { encoding: "utf8" },
  );
  const manifest = packedManifest(output.trim());
  const source = JSON.parse(readFileSync("packages/vite-plugin/package.json", "utf8"));

  for (const [name, requirement] of Object.entries(source.dependencies)) {
    if (requirement.startsWith("workspace:")) {
      assert.equal(manifest.dependencies[name], source.version);
    }
  }

  assert.doesNotMatch(JSON.stringify(manifest), /"workspace:/);
  assert.match(workflow, /node scripts\/pack-npm\.mjs/);
  assert.match(workflow, /node scripts\/publish-npm\.mjs --provenance --list/);
  assert.match(guide, /node scripts\/pack-npm\.mjs/);
  assert.match(guide, /node scripts\/publish-npm\.mjs --list/);
});

void test("registry publication resumes after a partial release", () => {
  assert.match(workflow, /run: node scripts\/publish-crates\.mjs/);
  assert.match(workflow, /node scripts\/publish-npm\.mjs --provenance --list/);
  assert.doesNotMatch(workflow, /for crate in .*publish-order/);
  assert.doesNotMatch(workflow, /npm publish "\$tarball"/);
  assert.match(guide, /node scripts\/publish-crates\.mjs/);
  assert.match(guide, /node scripts\/publish-npm\.mjs --list/);
});

test("the release packer refuses an unresolved workspace dependency", () => {
  assert.throws(
    () =>
      assertPublishableManifest(
        { dependencies: { "@example/internal": "workspace:*" } },
        "packages/example",
      ),
    /packages\/example: dependencies\.@example\/internal still uses workspace:\*/,
  );
});
