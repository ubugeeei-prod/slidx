/**
 * What `RELEASING.md` says about the registries, against what is true.
 *
 * The release procedure is followed once, by hand, under time pressure, and
 * every step of it is irreversible. So the parts of it that name something the
 * repository also names — the scope, the organisation that scope needs — are
 * checked here rather than left to be discovered as a 404 halfway through.
 */

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { publishedPackages } from "../licensed.mjs";

const releasing = readFileSync("RELEASING.md", "utf8");

/** The scope every scoped package publishes under. */
function scope() {
  const scopes = new Set(
    publishedPackages()
      .map((directory) => JSON.parse(readFileSync(`${directory}/package.json`, "utf8")).name)
      .filter((name) => name.startsWith("@"))
      .map((name) => name.split("/")[0]),
  );

  assert.equal(scopes.size, 1, `expected one scope, found ${[...scopes].join(", ")}`);
  return [...scopes][0];
}

test("the release procedure names the scope the packages actually use", () => {
  // A scope renamed in the manifests and not here sends whoever follows this
  // document to create the wrong organisation.
  assert.match(releasing, new RegExp(scope().replace("@", "@?")));
});

test("it says the scope needs an organisation, which no command here creates", () => {
  // An npm scope is either your username or an organisation. This one is
  // neither the publishing username nor anything `npm publish` creates on the
  // way past, and npm reports the gap as a 404 on the scope rather than as a
  // missing organisation.
  const organisation = scope().slice(1);

  assert.match(releasing, /organisation|organization/i);
  assert.match(releasing, new RegExp(`\`${organisation}\``));
});

test("it points at the workflow that builds the artifacts instead of a laptop", () => {
  // Cross-compiling five platforms by hand is the step most likely to be done
  // differently from how the release workflow does it.
  assert.match(releasing, /Registry bootstrap artifacts/);
});
