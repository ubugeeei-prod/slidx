/**
 * What blocks a build, and who decides.
 *
 * The linter assigns a severity to every finding, and the build has to turn
 * that into one yes or no. Both sides can answer — Rust reports `hasBlocking`,
 * and the plugin partitions the findings by severity to show them — so the risk
 * is not that either is wrong but that they drift, and a build that fails while
 * reporting no blocking diagnostics is the shape that would take an evening to
 * understand.
 *
 * So the decision is Rust's and the partition is for display, and these tests
 * hold the two to the same answer.
 */

import { describe, expect, it } from "vite-plus/test";

import { build } from "../src/pipeline";
import { groupFindings } from "../src/report";

/** A remote asset. The zero-network rule is one of the few that blocks. */
const REMOTE = "# One\n\n![a logo](https://example.com/logo.png)\n";

const CLEAN = "# One\n\n- one\n- two\n";

async function agreement(source: string) {
  const built = await build(source, {});
  const { blocking } = groupFindings(built.diagnostics);

  return { hasBlocking: built.hasBlocking, blocking: blocking.length };
}

describe("what blocks a build", () => {
  it("is reported by Rust for a deck that reaches the network", async () => {
    const { hasBlocking, blocking } = await agreement(REMOTE);

    expect(hasBlocking).toBe(true);
    expect(blocking).toBeGreaterThan(0);
  });

  it("is reported by neither side for a clean deck", async () => {
    const { hasBlocking, blocking } = await agreement(CLEAN);

    expect(hasBlocking).toBe(false);
    expect(blocking).toBe(0);
  });

  it("agrees with the severity partition, which is what stops the two drifting", async () => {
    for (const source of [REMOTE, CLEAN]) {
      const { hasBlocking, blocking } = await agreement(source);

      expect(hasBlocking, source).toBe(blocking > 0);
    }
  });
});
