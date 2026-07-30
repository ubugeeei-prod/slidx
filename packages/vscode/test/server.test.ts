import { readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vite-plus/test";

import {
  DECK_GLOB,
  LANGUAGE,
  SERVER_COMMAND,
  documentSelector,
  serverCommand,
} from "../src/server";

// A path rather than `new URL(…, import.meta.url)`: Vite rewrites that pattern
// during transform and the URL that arrives is no longer a file one.
const manifest = JSON.parse(readFileSync(join(import.meta.dirname, "../package.json"), "utf8")) as {
  activationEvents: string[];
  contributes: Record<string, unknown>;
  main: string;
};

describe("what the extension starts", () => {
  it("runs the language server as a subcommand of the one binary slidx ships", () => {
    // Not `slidx-lsp`. An extension that had to find a second binary would find
    // neither on every machine where only one install channel ran, and the
    // second one would sit outside `.slidx-version` entirely.
    expect(serverCommand("/usr/local/bin/slidx")).toEqual({
      command: "/usr/local/bin/slidx",
      args: ["lsp"],
    });
    expect(SERVER_COMMAND).toBe("lsp");
  });
});

describe("which documents it claims", () => {
  it("leaves them as Markdown rather than taking them over", () => {
    // A deck is Markdown. Registering a language of its own would take these
    // files from whatever Markdown tooling the author already has, in exchange
    // for a highlighter slidx would then have to write and keep in step with
    // enums a TextMate grammar cannot read.
    expect(documentSelector()).toEqual([
      { scheme: "file", language: "markdown", pattern: DECK_GLOB },
    ]);
    expect(LANGUAGE).toBe("markdown");
  });

  it("contributes no language, no grammar, and no file association", () => {
    const contributed = Object.keys(manifest.contributes);

    expect(contributed).toEqual(["configuration"]);
  });

  it("matches slide files and nothing else a workspace holds", () => {
    const matches = (path: string) => globMatches(DECK_GLOB, path);

    expect(matches("talks/vueconf/slides/0001.md")).toBe(true);
    expect(matches("slides/opening.md")).toBe(true);

    expect(matches("talks/vueconf/README.md")).toBe(false);
    expect(matches("notes/2026-07-30.md")).toBe(false);
    expect(matches("talks/slides/images/credits.md")).toBe(false);
    expect(matches("talks/old-slides/0001.md")).toBe(false);
    expect(matches("talks/slides/theme.css")).toBe(false);
  });

  it("only wakes up in a workspace that actually holds a deck", () => {
    // An extension that activated on every window would start a language
    // server for somebody writing a changelog.
    expect(manifest.activationEvents).toEqual([`workspaceContains:${DECK_GLOB}`]);
  });

  it("has exactly one setting, and it is the one nobody can guess for them", () => {
    const properties = (
      manifest.contributes["configuration"] as { properties: Record<string, unknown> }
    ).properties;

    expect(Object.keys(properties)).toEqual(["slidx.path"]);
  });

  it("is loaded as CommonJS, which is what an extension host requires", () => {
    // The package is `"type": "module"` like every other one here, so the
    // entry point has to say otherwise in its own extension.
    expect(manifest.main.endsWith(".cjs")).toBe(true);
  });
});

/**
 * Enough of a glob engine to state what the pattern means.
 *
 * VS Code's own matcher is not reachable from a test that does not run inside
 * an editor, and the claim being made is about the pattern rather than about
 * their implementation of it: `**` spans directories, `*` does not.
 */
function globMatches(glob: string, path: string): boolean {
  const expression = glob
    .split("/")
    .map((segment) => {
      if (segment === "**") return "(?:.*/)?";
      return `${segment.replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, "[^/]*")}/`;
    })
    .join("");

  return new RegExp(`^${expression.replace(/\/$/, "")}$`).test(path);
}
