/**
 * The documentation deployment's trust boundary.
 *
 * Deployment is allowed to mint one short-lived GitHub identity and nothing
 * long-lived. This test deliberately reads the workflow as text: adding a
 * secret, moving deployment before validation, or losing the static output
 * directory must be review-visible and test-visible at the same time.
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vite-plus/test";

const ROOT = join(import.meta.dirname, "../..");
const WORKFLOW = readFileSync(join(ROOT, ".github/workflows/docs.yml"), "utf8");

describe("the documentation deployment", () => {
  it("uses a pinned Void CLI over GitHub OIDC", () => {
    expect(WORKFLOW).toContain("id-token: write");
    expect(WORKFLOW).toContain('vpx void@0.10.11 deploy --dir docs/dist --project "$VOID_PROJECT"');
    expect(WORKFLOW).toContain('vpx() { pnpm exec vp dlx "$@"; }');
    expect(WORKFLOW).not.toContain("VOID_TOKEN");
    expect(WORKFLOW).not.toContain("secrets.");
  });

  it("checks and builds the same output it deploys", () => {
    const check = WORKFLOW.indexOf("cargo test -p slidx_docs");
    const build = WORKFLOW.indexOf("vp run docs:build");
    const deploy = WORKFLOW.indexOf("vpx void@");

    expect(check).toBeGreaterThan(0);
    expect(build).toBeGreaterThan(check);
    expect(deploy).toBeGreaterThan(build);
  });

  it("has no second publishing path", () => {
    for (const old of ["actions/configure-pages", "actions/upload-pages", "actions/deploy-pages"]) {
      expect(WORKFLOW, old).not.toContain(old);
    }
  });
});
