/**
 * Publishes the workspace to crates.io, waiting out the rate limit.
 *
 * crates.io meters **new crate names**: a burst, then one per window. A
 * workspace this size therefore cannot be published in one pass, and the plain
 * loop in `RELEASING.md` stops at the first refusal — correctly, because
 * continuing turns one real error into a dozen downstream ones, but it leaves a
 * person re-running a command every ten minutes for an hour.
 *
 * So this waits. The refusal carries the exact time to try again, and that is
 * the only thing it sleeps on: no interval is guessed here, and nothing is
 * retried that was not refused for that reason.
 *
 * It is resumable by construction. A crate already on the registry at this
 * version is skipped, so an interrupted run is continued by running it again —
 * which matters more than usual, because the half of a release that is already
 * published cannot be taken back.
 *
 * ```sh
 * node scripts/publish-crates.mjs            # publish, waiting as required
 * node scripts/publish-crates.mjs --dry-run  # say what it would do
 * ```
 */

import { execFileSync, spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { setTimeout as sleep } from "node:timers/promises";

const dryRun = process.argv.includes("--dry-run");

/** The one version everything in the workspace is published at. */
function version() {
  const manifest = readFileSync("Cargo.toml", "utf8");
  const section = manifest.slice(manifest.indexOf("[workspace.package]"));
  const found = /^version\s*=\s*"([^"]+)"/m.exec(section);

  if (!found) throw new Error("no version in [workspace.package]");
  return found[1];
}

const order = execFileSync("node", ["scripts/publish-order.mjs", "crates"], { encoding: "utf8" })
  .split("\n")
  .filter(Boolean);

const release = version();

/** Whether the registry already has this crate at this version. */
async function published(crate) {
  const response = await fetch(`https://crates.io/api/v1/crates/${crate}`, {
    headers: { "user-agent": `slidx release ${release} (https://github.com/ubugeeei-prod/slidx)` },
  });

  if (response.status === 404) return false;
  if (!response.ok) throw new Error(`crates.io answered ${response.status} about ${crate}`);

  const body = await response.json();
  return (body.versions ?? []).some((entry) => entry.num === release);
}

/**
 * When the registry says to try again, if that is why it refused.
 *
 * Read out of the message rather than computed, because the window is theirs to
 * decide and a number invented here would be wrong the day they change it.
 */
function tryAgainAt(output) {
  if (!/429|too many/i.test(output)) return undefined;

  const found = /try again after ([^,]+, [^)\n]+?GMT)/i.exec(output);
  const at = found ? Date.parse(found[1]) : Number.NaN;

  return Number.isNaN(at) ? undefined : at;
}

for (const crate of order) {
  if (await published(crate)) {
    process.stdout.write(`  ${crate} ${release} is already there\n`);
    continue;
  }

  if (dryRun) {
    process.stdout.write(`  ${crate} would be published\n`);
    continue;
  }

  for (;;) {
    process.stdout.write(`\n$ cargo publish -p ${crate}\n`);
    const result = spawnSync("cargo", ["publish", "-p", crate], { encoding: "utf8" });
    const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
    process.stdout.write(output);

    if (result.status === 0) break;

    const at = tryAgainAt(output);
    if (at === undefined) {
      process.stderr.write(
        `\nerror: ${crate} was refused for a reason that is not the rate limit\n`,
      );
      process.exit(1);
    }

    // A little past the stated moment, because two clocks are involved and the
    // cost of being a second early is another whole window.
    const waiting = Math.max(0, at + 5_000 - Date.now());
    process.stdout.write(
      `\nrate limited. ${crate} again at ${new Date(at).toISOString()} ` +
        `(${Math.ceil(waiting / 60_000)} min)\n`,
    );
    await sleep(waiting);
  }
}

process.stdout.write(`\ncrates: ${order.length} at ${release}\n`);
