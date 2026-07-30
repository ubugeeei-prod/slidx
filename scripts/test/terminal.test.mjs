/**
 * A recording task must be able to capture startup from a long-lived command
 * without leaking that command. Exercise the real pty boundary: this is the
 * behaviour the CLI tour relies on, not a mock of child-process events.
 */

import { describe, expect, it } from "vite-plus/test";

import { captureUntil, toHtml } from "../terminal.mjs";

describe.skipIf(process.platform === "win32")("a live terminal capture", () => {
  it("returns startup once the command is ready and stops the process group", async () => {
    const output = await captureUntil(
      process.execPath,
      ["-e", "console.log('editor ready'); setInterval(() => {}, 1_000)"],
      { cwd: import.meta.dirname, until: /editor ready/, timeout: 2_000 },
    );

    expect(output).toContain("editor ready");
  });

  it("fails instead of leaving a command alive when readiness never arrives", async () => {
    await expect(
      captureUntil(process.execPath, ["-e", "setInterval(() => {}, 1_000)"], {
        cwd: import.meta.dirname,
        until: /never printed/,
        timeout: 50,
      }),
    ).rejects.toThrow("before the timeout");
  });
});

describe("terminal markup", () => {
  it("escapes output while preserving the CLI emphasis it captured", () => {
    expect(toHtml("\u001b[1m<ready>\u001b[0m")).toBe('<span class="t-bold">&lt;ready&gt;</span>');
  });
});
