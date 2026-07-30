import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { PLATFORMS } from "../platforms.mjs";

const workflow = readFileSync(".github/workflows/registry-bootstrap.yml", "utf8");

test("registry bootstrap only creates reviewable artifacts", () => {
  assert.match(workflow, /workflow_dispatch:/);
  assert.match(workflow, /ref: \$\{\{ inputs\.ref \}\}/);
  assert.match(workflow, /node scripts\/pack-npm\.mjs/);
  assert.match(workflow, /bundle="\$GITHUB_WORKSPACE\/bundle"/);
  assert.match(workflow, /path: \$\{\{ github\.workspace \}\}\/bundle/);
  assert.match(workflow, /publish-order\.txt/);
  assert.match(workflow, /uses: actions\/upload-artifact@v4/);
  assert.doesNotMatch(workflow, /id-token:\s*write/);
  assert.doesNotMatch(workflow, /\b(?:cargo|npm) publish\b/);
});

test("registry bootstrap builds every supported platform on its strongest runner", () => {
  for (const platform of PLATFORMS) {
    assert.match(
      workflow,
      new RegExp(`target: ${platform.target}\\n\\s+runner: ${platform.runner}`),
    );
  }
});
