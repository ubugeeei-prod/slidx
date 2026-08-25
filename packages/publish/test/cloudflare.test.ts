/**
 * Cloudflare Pages, as a file rather than a login.
 *
 * The destination exists to remove the chore of inventing a project name, a
 * compatibility date, and a build-output path. What it must never do is hold a
 * token or run wrangler — a tool that can deploy as you is a tool that has to
 * be trusted with a credential.
 */

import { describe, expect, it } from "vite-plus/test";

import { composeCloudflare, describeCloudflare } from "../src/targets/cloudflare";
import type { Composed } from "../src/types";
import { TALK } from "./support";

function fieldsOf(result: Composed<unknown>): string[] {
  return result.ok ? [] : result.reasons.map((reason) => reason.field);
}

describe("a latin title", () => {
  it("becomes the Pages project name and a wrangler.toml", () => {
    const result = composeCloudflare({ meta: TALK });

    expect(result.ok).toBe(true);
    if (!result.ok) return;

    expect(result.value.name).toBe("zero-javascript-slides");
    expect(result.value.path).toBe("wrangler.toml");
    expect(result.value.command).toBe("wrangler pages deploy");
    expect(result.value.toml).toContain('name = "zero-javascript-slides"');
    expect(result.value.toml).toContain('pages_build_output_dir = "./dist"');
    expect(result.value.toml).toContain('compatibility_date = "2026-08-25"');
  });

  it("summarises the file and the command the author still has to run", () => {
    const result = composeCloudflare({ meta: TALK });

    expect(result.ok).toBe(true);
    if (!result.ok) return;

    expect(describeCloudflare(result.value)).toBe(
      "write wrangler.toml; then wrangler pages deploy",
    );
  });

  it("names no token and asks for no login from slidx", () => {
    const result = composeCloudflare({ meta: TALK });

    expect(result.ok).toBe(true);
    if (!result.ok) return;

    expect(result.value.toml.toUpperCase()).not.toMatch(/(?:^|\n)[^#\n]*TOKEN/i);
    expect(result.value.toml).toContain("`wrangler login` is yours");
  });
});

describe("a name the author pinned", () => {
  it("is the project name rather than the title", () => {
    const result = composeCloudflare({
      meta: { ...TALK, slug: "plain-html" },
    });

    expect(result.ok && result.value.name).toBe("plain-html");
  });
});

describe("a title with no latin characters", () => {
  it("is blocked rather than invented", () => {
    const result = composeCloudflare({ meta: { title: "日本語のスライド" } });

    expect(fieldsOf(result)).toEqual(["slug"]);
    expect(result.ok ? "" : result.reasons[0]?.message).toContain("`slug:`");
  });
});
