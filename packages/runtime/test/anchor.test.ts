/**
 * The anchor contract.
 *
 * A step anchor is an empty `<span data-slidx-step="N" hidden>` that the core
 * compiler leaves in the Markdown. Any Markdown renderer will carry it through
 * to the output — that is what keeps slidx framework-agnostic — but where it
 * lands depends on how the author wrote it. Three positions are possible, and
 * `resolveAnchor` has to turn each into the element the author meant.
 *
 * These tests are the specification. `crates/slidx_core/src/markers.rs`
 * documents the same three cases from the authoring side.
 */

import { beforeEach, describe, expect, it } from "vitest";

import { ANCHOR_ATTRIBUTE, findAnchors, resolveAnchor } from "../src/anchor";

function mount(html: string): HTMLElement {
  const root = document.createElement("div");
  root.className = "slidx-slide";
  root.innerHTML = html;
  document.body.replaceChildren(root);
  return root;
}

function anchor(root: HTMLElement, id: number): HTMLElement {
  const found = root.querySelector<HTMLElement>(`[${ANCHOR_ATTRIBUTE}="${id}"]`);
  if (!found) throw new Error(`no anchor ${id} in: ${root.innerHTML}`);
  return found;
}

beforeEach(() => {
  document.body.replaceChildren();
});

describe("case 1 — an anchor alone in its block", () => {
  it("stages the previous element sibling", () => {
    const root = mount(
      `<pre><code>fn main() {}</code></pre><p><span ${ANCHOR_ATTRIBUTE}="1" hidden></span></p>`,
    );

    expect(resolveAnchor(root, anchor(root, 1))?.tagName).toBe("PRE");
  });

  it("removes the wrapper it was alone in", () => {
    // The wrapper is an artifact of the marker syntax. Leaving it in place
    // would add an empty paragraph to the slide's vertical rhythm.
    const root = mount(`<p>Prose.</p><p><span ${ANCHOR_ATTRIBUTE}="1" hidden></span></p>`);
    resolveAnchor(root, anchor(root, 1));

    expect(root.querySelectorAll("p")).toHaveLength(1);
  });

  it("stages a table, an image, or anything else that precedes it", () => {
    for (const block of [
      "<table><tbody><tr><td>a</td></tr></tbody></table>",
      "<blockquote>q</blockquote>",
    ]) {
      const root = mount(`${block}<p><span ${ANCHOR_ATTRIBUTE}="1" hidden></span></p>`);
      const staged = resolveAnchor(root, anchor(root, 1));

      expect(staged).not.toBeNull();
      expect(staged?.tagName).toBe(block.startsWith("<table") ? "TABLE" : "BLOCKQUOTE");
    }
  });

  it("resolves to nothing when there is no preceding block", () => {
    // A marker before any content has nothing to stage. Returning null lets
    // the caller drop the step rather than staging the whole slide.
    const root = mount(`<p><span ${ANCHOR_ATTRIBUTE}="1" hidden></span></p>`);
    expect(resolveAnchor(root, anchor(root, 1))).toBeNull();
  });

  it("treats whitespace around the anchor as empty", () => {
    const root = mount(`<p>Prose.</p><p>\n  <span ${ANCHOR_ATTRIBUTE}="1" hidden></span>\n</p>`);
    expect(resolveAnchor(root, anchor(root, 1))?.textContent).toBe("Prose.");
  });
});

describe("case 2 — an anchor at the end of a list item", () => {
  it("stages the list item, not the list", () => {
    const root = mount(
      `<ul><li>one<span ${ANCHOR_ATTRIBUTE}="1" hidden></span></li><li>two</li></ul>`,
    );
    const staged = resolveAnchor(root, anchor(root, 1));

    expect(staged?.tagName).toBe("LI");
    expect(staged?.textContent).toBe("one");
  });

  it("stages the item even when the list is loose and the text is wrapped", () => {
    // A loose list puts a <p> between the <li> and the text, which would
    // otherwise be the closest block and stage only the paragraph.
    const root = mount(
      `<ul><li><p>one<span ${ANCHOR_ATTRIBUTE}="1" hidden></span></p></li><li><p>two</p></li></ul>`,
    );

    expect(resolveAnchor(root, anchor(root, 1))?.tagName).toBe("LI");
  });

  it("stages the innermost item when lists are nested", () => {
    const root = mount(
      `<ul><li>outer<ul><li>inner<span ${ANCHOR_ATTRIBUTE}="1" hidden></span></li></ul></li></ul>`,
    );

    expect(resolveAnchor(root, anchor(root, 1))?.textContent).toBe("inner");
  });

  it("stages a table row when the anchor is in one", () => {
    const root = mount(
      `<table><tbody><tr><td>1<span ${ANCHOR_ATTRIBUTE}="1" hidden></span></td></tr></tbody></table>`,
    );

    expect(resolveAnchor(root, anchor(root, 1))?.tagName).toBe("TR");
  });
});

describe("case 3 — an anchor inside ordinary prose", () => {
  it("stages the closest block that is a direct child of the slide", () => {
    const root = mount(`<p>Some prose <span ${ANCHOR_ATTRIBUTE}="1" hidden></span></p>`);
    const staged = resolveAnchor(root, anchor(root, 1));

    expect(staged?.tagName).toBe("P");
    expect(staged?.parentElement).toBe(root);
  });

  it("climbs out of inline wrappers", () => {
    const root = mount(
      `<p>See <strong>this <span ${ANCHOR_ATTRIBUTE}="1" hidden></span></strong></p>`,
    );
    expect(resolveAnchor(root, anchor(root, 1))?.tagName).toBe("P");
  });

  it("stages a heading", () => {
    const root = mount(`<h2>Title <span ${ANCHOR_ATTRIBUTE}="1" hidden></span></h2>`);
    expect(resolveAnchor(root, anchor(root, 1))?.tagName).toBe("H2");
  });
});

describe("finding anchors", () => {
  it("returns them in document order", () => {
    const root = mount(
      `<p>a<span ${ANCHOR_ATTRIBUTE}="1" hidden></span></p>` +
        `<p>b<span ${ANCHOR_ATTRIBUTE}="2" hidden></span></p>`,
    );

    expect(findAnchors(root).map((element) => element.getAttribute(ANCHOR_ATTRIBUTE))).toEqual([
      "1",
      "2",
    ]);
  });

  it("is scoped to its slide", () => {
    // Anchor ids restart on every slide, so a deck rendered as one document
    // would collide if the query were not scoped.
    const first = document.createElement("div");
    first.innerHTML = `<p>a<span ${ANCHOR_ATTRIBUTE}="1" hidden></span></p>`;
    const second = document.createElement("div");
    second.innerHTML = `<p>b<span ${ANCHOR_ATTRIBUTE}="1" hidden></span></p>`;
    document.body.replaceChildren(first, second);

    expect(findAnchors(first)).toHaveLength(1);
    expect(resolveAnchor(first, findAnchors(first)[0]!)?.textContent).toBe("a");
  });

  it("finds nothing in a slide with no steps", () => {
    expect(findAnchors(mount("<p>Just prose.</p>"))).toHaveLength(0);
  });
});
