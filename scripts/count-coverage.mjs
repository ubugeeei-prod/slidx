/**
 * The numbers ROADMAP.md publishes about itself.
 *
 * That table calls itself "the honest measure", but its handwritten totals
 * have drifted more than once. A count written by hand is only true on the
 * afternoon it was typed, so this command asks the test runners and manifests
 * again instead of preserving another stale number in its own explanation.
 *
 * So it is measured instead, on the same principle as the build-time table the
 * README publishes: `node scripts/bench-build.mjs` reproduces those, so the
 * number is measured rather than remembered. This is that command for the
 * coverage counts.
 *
 * Not a check. A count that failed CI whenever somebody added a test would be a
 * tax on the thing it is meant to encourage — this prints, and a person pastes
 * it when they are already editing the document.
 */

import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";

function run(command, args) {
  // Test runners narrate on stderr, and the only thing wanted here is the
  // count on stdout.
  return execFileSync(command, args, {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    stdio: ["ignore", "pipe", "ignore"],
  });
}

/**
 * Every test cargo would run, asked of cargo rather than counted from source.
 *
 * `#[test]` attributes would undercount: a property test is one attribute over
 * thousands of generated cases, and this workspace leans on those.
 */
function rustTests() {
  return run("cargo", ["test", "--workspace", "--", "--list"])
    .split("\n")
    .filter((line) => line.endsWith(": test")).length;
}

/** Every test the TypeScript runner collects, asked of the runner. */
function typescriptTests() {
  const output = run("vp", ["test", "--run", "--reporter=json"]);
  const start = output.indexOf("{");

  return JSON.parse(output.slice(start)).numTotalTests;
}

const crates = readdirSync("crates").length;
const publishablePackages = readdirSync("packages").filter((name) => {
  const manifest = JSON.parse(readFileSync(`packages/${name}/package.json`, "utf8"));
  return !manifest.private;
}).length;

process.stdout.write(
  [
    `| Rust tests          | ${rustTests()} |`,
    `| TypeScript tests    | ${typescriptTests()} |`,
    `| Crates              | ${crates} |`,
    `| Publishable npm packages | ${publishablePackages} |`,
    "",
  ].join("\n"),
);
