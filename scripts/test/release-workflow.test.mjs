import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const workflow = readFileSync(".github/workflows/release.yml", "utf8");

test("the npm release builds the package graph from the repository root", () => {
  assert.match(workflow, /with:\n\s+wasm: "true"/);
  assert.match(workflow, /run: pnpm exec vp run build:packages/);
  assert.doesNotMatch(workflow, /pnpm -r .* run pack:lib/);
});
