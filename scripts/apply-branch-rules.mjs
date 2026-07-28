/**
 * Applies the branch ruleset in `.github/ruleset-main.json`.
 *
 * Kept as a file rather than clicked into the settings UI so the rules are
 * reviewable, diffable, and restorable — the same reason everything else here
 * is a file.
 *
 * Requires the repository to be public, or the organization to be on a paid
 * plan: GitHub rejects rulesets on private repositories under a free
 * organization, which is where this currently stands.
 */

import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";

const REPO = process.env.SLIDX_REPO ?? "ubugeeei-prod/slidx";
const ruleset = readFileSync(".github/ruleset-main.json", "utf8");

const existing = JSON.parse(gh(["api", `repos/${REPO}/rulesets`])).find(
  (candidate) => candidate.name === JSON.parse(ruleset).name,
);

const [method, path] = existing
  ? ["PUT", `repos/${REPO}/rulesets/${existing.id}`]
  : ["POST", `repos/${REPO}/rulesets`];

const result = JSON.parse(gh(["api", "-X", method, path, "--input", "-"], ruleset));
process.stdout.write(`${existing ? "updated" : "created"} ruleset ${result.name} (${result.id})\n`);

// Auto-merge is a repository setting rather than part of the ruleset, but it
// is only useful alongside one: without required checks there is nothing for a
// queued merge to wait on.
gh(["api", "-X", "PATCH", `repos/${REPO}`, "-F", "allow_auto_merge=true"]);
process.stdout.write("auto-merge enabled\n");

function gh(args, input) {
  try {
    return execFileSync("gh", args, { encoding: "utf8", input });
  } catch (error) {
    process.stderr.write(`${error.stderr || error.message}\n`);
    process.exit(1);
  }
}
