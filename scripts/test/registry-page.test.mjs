/**
 * The page a registry shows, and the one thing about it that is checked.
 *
 * `check-pages.mjs` reads a page's first heading and nothing else, because that
 * is the part that goes wrong invisibly: a page copied from the package next
 * door reads perfectly and is about something else.
 */

import { describe, expect, it } from "vite-plus/test";

import { needsCommitted } from "../licensed.mjs";
import { firstHeading, registryPage } from "../registry-page.mjs";

describe("composing a page", () => {
  const page = registryPage({
    name: "slidx_core",
    description: "Deck model, Markdown deck parsing, and the step pipeline",
  });

  it("titles it with the name the thing is published under", () => {
    expect(firstHeading(page)).toBe("slidx_core");
  });

  it("opens with the description the manifest already carries", () => {
    // From the manifest rather than from a table here, so a page can only
    // describe a crate as something other than what it is if its own
    // `description` already does.
    expect(page).toContain("Deck model, Markdown deck parsing, and the step pipeline.");
  });

  it("leaves a description that ends in a full stop alone", () => {
    expect(registryPage({ name: "one", description: "Already a sentence." })).toContain(
      "Already a sentence.\n",
    );
    expect(registryPage({ name: "one", description: "Already a sentence." })).not.toContain("..");
  });

  it("names both front doors, whichever page a reader arrived at", () => {
    // Somebody who reached an edit-operation model from a search result has
    // almost certainly not gone looking for one. They want to make a deck.
    expect(page).toContain("npm i -D @slidxjs/vite-plugin");
    expect(page).toContain("npm i -g slidx");
  });

  it("says where the notice is, in the package and in the repository", () => {
    expect(page).toContain("MIT.");
    expect(page).toContain("/blob/main/LICENSE");
  });
});

describe("reading what a page is about", () => {
  it("takes the first heading, not a later one", () => {
    expect(firstHeading("# slidx_qr\n\nSomething.\n\n# Not this\n")).toBe("slidx_qr");
  });

  it("ignores a heading deeper than the title", () => {
    expect(firstHeading("## License\n")).toBeUndefined();
  });

  it("finds nothing in a page with no heading at all", () => {
    expect(firstHeading("Just some prose.\n")).toBeUndefined();
  });

  it("trims what a formatter may have left behind it", () => {
    expect(firstHeading("#   @slidxjs/runtime   \n")).toBe("@slidxjs/runtime");
  });
});

describe("asking about a page the same way the licence does", () => {
  it("uses one rule for both files, so they cannot disagree about a directory", () => {
    // `packages/wasm` is emptied and refilled by its build, which writes its
    // page and copies the notice in beside it. Two rules would eventually
    // answer differently about the same directory.
    const ignoredPage = new Set(["packages/wasm/README.md"]);

    expect(
      needsCommitted(["packages/one", "packages/wasm"], "README.md", () => ignoredPage),
    ).toEqual(["packages/one"]);
  });

  it("keeps every directory whose page git has not been told to ignore", () => {
    expect(needsCommitted(["packages/one"], "README.md", () => new Set())).toEqual([
      "packages/one",
    ]);
  });
});
