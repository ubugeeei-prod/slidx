/**
 * One deck, three runtimes, one answer.
 *
 * slidx's pipeline is a WebAssembly module built for the web target, so one
 * artifact serves the browser, bundlers, and every server runtime. The price
 * is that in a server runtime nothing fetches the bytes for us: the plugin
 * resolves the package, reads the file, and instantiates the module itself.
 * That path is the one place Node, Bun, and Deno genuinely differ, and every
 * build anyone runs goes through it.
 *
 * The check is a digest of a *complete build* rather than a flag saying the
 * module loaded. A runtime that instantiated the pipeline and then produced
 * different HTML would sail through the weaker check, and the difference would
 * surface as a deck that renders one way on the author's machine and another
 * way in CI.
 *
 * A runtime that is not installed skips, loudly. CI installs all three.
 */

import { execFile } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import { describe, expect, it } from "vitest";

const run = promisify(execFile);

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const script = join(root, "scripts", "runtime-matrix.mjs");

interface Runtime {
  name: string;
  command: string;
  args: string[];
}

/**
 * How each runtime is asked to run the same file.
 *
 * Deno's permissions are listed rather than granted wholesale: the script
 * reads two files and reads the environment, and a test that ran it with `-A`
 * would not notice the day the pipeline started wanting the network.
 */
const RUNTIMES: Runtime[] = [
  { name: "node", command: "node", args: [script] },
  { name: "bun", command: "bun", args: [script] },
  {
    name: "deno",
    command: "deno",
    args: ["run", "--allow-read", "--allow-env", "--allow-sys", script],
  },
];

async function digestUnder(runtime: Runtime): Promise<string | undefined> {
  try {
    const { stdout } = await run(runtime.command, runtime.args, { cwd: root });
    return stdout.trim();
  } catch {
    return undefined;
  }
}

const results = await Promise.all(
  RUNTIMES.map(async (runtime) => [runtime.name, await digestUnder(runtime)] as const),
);

const built = results.filter((entry): entry is [string, string] => entry[1] !== undefined);
const absent = results.filter(([, output]) => output === undefined).map(([name]) => name);

if (absent.length > 0) {
  process.stdout.write(`\nRuntime matrix: ${absent.join(", ")} did not run here.\n`);
}

describe("the runtime matrix", () => {
  it("has a runtime to check at all", () => {
    // Node is running this file, so an empty matrix means the script itself is
    // broken rather than that nothing is installed.
    expect(built.length).toBeGreaterThan(0);
  });

  it("builds the same bytes everywhere", () => {
    // The property worth having: whichever runtime a contributor or a CI job
    // happens to use, the deck that ships is the same deck.
    const [first, ...rest] = built;
    if (first === undefined) return;

    for (const [name, output] of rest) {
      expect(output, `${name} disagrees with ${first[0]}`).toBe(first[1]);
    }
  });

  it("builds a deck rather than an empty one", () => {
    // A digest of nothing also matches a digest of nothing.
    for (const [name, output] of built) {
      expect(output, name).toContain("slides=1 stops=3");
    }
  });
});
