/**
 * Editing a slide's words where they are drawn.
 *
 * Every one of these asserts the `EditOp` the gesture produced and the bytes it
 * names, because the whole claim is that a caret on the canvas addresses the
 * Markdown: a test that only checked the words would pass while the edit landed
 * three characters to the left.
 *
 * The block spans in the fixtures are what `slidx_edit::slide_spans` reports for
 * the same source — body-local, blocks in source order, a mark's words apart
 * from the group that addresses them. `crates/slidx_edit/src/spans.rs` is what
 * holds the two ends together.
 */

import { afterEach, describe, expect, it } from "vite-plus/test";

import { attachEditing, EDITING_STYLESHEET } from "../src/canvas";
import type { BlockSpans } from "../src/client";
import type { EditOp } from "../src/operations";
import { changeBetween, editableIn, planBlock, rangeOf } from "../src/text";

/** A deck body, the page the pipeline renders it as, and where its blocks are. */
interface Fixture {
  body: string;
  html: string;
  blocks: BlockSpans[];
}

function page(fixture: Fixture): { document: Document; ops: EditOp[] } {
  const ops: EditOp[] = [];
  const page = window.document.implementation.createHTMLDocument();
  page.body.innerHTML = `<div class="slidx-slide-body">${fixture.html}</div>`;

  attachEditing(
    page,
    1,
    {
      run: (op) => {
        ops.push(op);
      },
      selected: () => {},
    },
    { body: () => fixture.body, blocks: () => fixture.blocks },
  );

  return { document: page, ops };
}

/** One block, wrapped the way `slidx_render::region` wraps it. */
function block(index: number, inner: string): string {
  return `<div class="slidx-block" data-slidx-block="${index}">${inner}</div>`;
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/** The bytes `needle` occupies in `text`, which is what a span here is. */
function span(text: string, needle: string): { start: number; end: number } {
  const start = encoder.encode(text.slice(0, text.indexOf(needle))).length;

  return { start, end: start + encoder.encode(needle).length };
}

/**
 * The body with one text operation's range spliced.
 *
 * A splice and nothing else, which is what makes it safe to write here: what a
 * *mark* the range reaches becomes is one rule and it lives in `slidx_edit`, so
 * this is only used on ranges that touch no mark.
 */
function applied(body: string, op: EditOp): string {
  if (op.op !== "setText") throw new Error(`${op.op} is not a text edit`);
  const bytes = encoder.encode(body);

  return (
    decoder.decode(bytes.subarray(0, op.range.start)) +
    op.text +
    decoder.decode(bytes.subarray(op.range.end))
  );
}

/** Types into a line and leaves it, which is what commits. */
function retype(element: Element, text: string): void {
  element.textContent = text;
  element.dispatchEvent(new window.Event("blur"));
}

const HEADING_AND_PROSE: Fixture = {
  body: "##   Two\n\nThe result was faster.",
  html: block(0, "<h2>Two</h2>") + block(1, "<p>The result was faster.</p>"),
  blocks: [{ span: { start: 0, end: 8 } }, { span: { start: 10, end: 32 } }],
};

afterEach(() => document.body.replaceChildren());

describe("typing on the canvas", () => {
  it("selects a whole block from the real rendered page", () => {
    const selected: number[] = [];
    const page = window.document.implementation.createHTMLDocument();
    page.body.innerHTML = `<div class="slidx-slide-body">${HEADING_AND_PROSE.html}</div>`;
    attachEditing(
      page,
      1,
      {
        run: () => {},
        selected: () => {},
        selectedBlock: (block) => {
          if (block !== undefined) selected.push(block);
        },
      },
      { body: () => HEADING_AND_PROSE.body, blocks: () => HEADING_AND_PROSE.blocks },
    );

    page
      .querySelector("p")!
      .dispatchEvent(new window.PointerEvent("pointerdown", { bubbles: true }));

    expect(selected).toEqual([1]);
  });

  it("selects across the iframe's separate element realm", () => {
    const selected: number[] = [];
    const frame = document.createElement("iframe");
    document.body.append(frame);
    const page = frame.contentDocument!;
    page.body.innerHTML = `<div class="slidx-slide-body">${HEADING_AND_PROSE.html}</div>`;
    attachEditing(
      page,
      1,
      {
        run: () => {},
        selected: () => {},
        selectedBlock: (block) => {
          if (block !== undefined) selected.push(block);
        },
      },
      { body: () => HEADING_AND_PROSE.body, blocks: () => HEADING_AND_PROSE.blocks },
    );

    page
      .querySelector("p")!
      .dispatchEvent(new window.PointerEvent("pointerdown", { bubbles: true }));

    expect(selected).toEqual([1]);
  });

  it("keeps block selection but opens no editable line for a view-only link", () => {
    const selected: number[] = [];
    const page = window.document.implementation.createHTMLDocument();
    page.body.innerHTML = `<div class="slidx-slide-body">${HEADING_AND_PROSE.html}</div>`;
    attachEditing(
      page,
      1,
      {
        run: () => {},
        selected: () => {},
        selectedBlock: (block) => {
          if (block !== undefined) selected.push(block);
        },
      },
      { body: () => HEADING_AND_PROSE.body, blocks: () => HEADING_AND_PROSE.blocks },
      false,
    );

    page
      .querySelector("p")!
      .dispatchEvent(new window.PointerEvent("pointerdown", { bubbles: true }));

    expect(page.querySelector("[contenteditable]")).toBeNull();
    expect(selected).toEqual([1]);
  });

  it("uses one offset hairline for editing instead of a layout-changing border", () => {
    const { document } = page(HEADING_AND_PROSE);
    const injected = document.querySelector<HTMLStyleElement>("style[data-slidx-editing]");

    expect(injected?.textContent).toBe(EDITING_STYLESHEET);
    expect(EDITING_STYLESHEET).toContain("outline: 1px solid transparent");
    expect(EDITING_STYLESHEET).toContain("outline-offset: 6px");
    expect(EDITING_STYLESHEET).not.toContain("border:");
  });

  it("retitles a heading and leaves the marker the author spaced out", () => {
    const { document, ops } = page(HEADING_AND_PROSE);
    const heading = document.querySelector("h2")!;

    expect(heading.getAttribute("contenteditable")).toBe("true");
    retype(heading, "Three");

    expect(ops.length).toBe(1);
    expect(applied(HEADING_AND_PROSE.body, ops[0]!)).toBe("##   Three\n\nThe result was faster.");
  });

  it("names only the word that changed in a paragraph", () => {
    // Not the paragraph. The splice is what a merge and a press of undo are
    // measured in, so a whole-paragraph range would take a co-author's sentence
    // back with it.
    const { document, ops } = page(HEADING_AND_PROSE);

    retype(document.querySelector("p")!, "The result was quicker.");

    expect(ops).toEqual([
      { op: "setText", slide: 1, range: span(HEADING_AND_PROSE.body, "fast"), text: "quick" },
    ]);
  });

  it("leaves a line emptied by accident alone", () => {
    // Deleting a block is an operation of its own, and a stray Backspace on the
    // last word of a heading should not be the way to reach it.
    const { document, ops } = page(HEADING_AND_PROSE);

    retype(document.querySelector("h2")!, "  ");

    expect(ops).toEqual([]);
  });

  it("writes nothing when a line was entered and left unchanged", () => {
    const { document, ops } = page(HEADING_AND_PROSE);

    document.querySelector("p")!.dispatchEvent(new window.Event("blur"));

    expect(ops).toEqual([]);
  });

  it("commits on Enter rather than letting a second line into one line of the file", () => {
    const { document, ops } = page(HEADING_AND_PROSE);
    const heading = document.querySelector("h2")!;

    heading.textContent = "Retitled";
    const event = new window.KeyboardEvent("keydown", { key: "Enter", cancelable: true });
    heading.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    expect(applied(HEADING_AND_PROSE.body, ops[0]!)).toBe(
      "##   Retitled\n\nThe result was faster.",
    );
  });

  it("names the words inside a mark rather than the mark, so its key survives", () => {
    // The failure this prevents is silent: the slide still reads correctly and
    // the `steps:` entry that animated `#latency` now targets nothing.
    const body = "Latency dropped to [120ms]{#latency}.";
    const { document, ops } = page({
      body,
      html: block(0, '<p>Latency dropped to <span data-slidx-mark="latency">120ms</span>.</p>'),
      blocks: [
        {
          span: { start: 0, end: body.length },
          marks: [{ span: span(body, "[120ms]{#latency}"), words: span(body, "120ms") }],
        },
      ],
    });

    retype(document.querySelector("p")!, "Latency dropped to 38ms.");

    expect(ops).toEqual([{ op: "setText", slide: 1, range: span(body, "120"), text: "38" }]);
  });

  it("names a range that crosses a mark's edge and lets the pipeline decide", () => {
    // The editor's job ends at the range: the bytes the two runs gave up. What
    // a mark left holding half its words becomes is one rule, and it is in
    // `slidx_edit`.
    const body = "Down to [120ms]{#latency} today.";
    const { document, ops } = page({
      body,
      html: block(0, '<p>Down to <span data-slidx-mark="latency">120ms</span> today.</p>'),
      blocks: [
        {
          span: { start: 0, end: body.length },
          marks: [{ span: span(body, "[120ms]{#latency}"), words: span(body, "120ms") }],
        },
      ],
    });

    retype(document.querySelector("p")!, "Down to 120 tomorrow.");

    expect(ops).toEqual([
      { op: "setText", slide: 1, range: { start: 12, end: 31 }, text: " tomorrow" },
    ]);
    expect(body.slice(12, 31)).toBe("ms]{#latency} today");
  });

  it("reaches the words inside emphasis without touching the asterisks", () => {
    // `**` in front of a word is syntax, so it is in front of the run rather
    // than in it — and a technical deck is mostly bold words and inline code.
    const body = "The **fast** path uses `mmap`.";
    const { document, ops } = page({
      body,
      html: block(0, "<p>The <strong>fast</strong> path uses <code>mmap</code>.</p>"),
      blocks: [{ span: { start: 0, end: body.length } }],
    });

    retype(document.querySelector("p")!, "The quick path uses mmap.");

    expect(ops.length).toBe(1);
    expect(applied(body, ops[0]!)).toBe("The **quick** path uses `mmap`.");
  });

  it("addresses the second of two identical bullets rather than the first", () => {
    // Matched forward, in reading order. Matching by text alone would send a
    // change to the second bullet into the first one.
    const body = "- again\n- again";
    const { document, ops } = page({
      body,
      html: block(0, "<ul><li>again</li><li>again</li></ul>"),
      blocks: [{ span: { start: 0, end: body.length } }],
    });

    retype(document.querySelectorAll("li")[1]!, "and again");

    expect(ops.length).toBe(1);
    expect(applied(body, ops[0]!)).toBe("- again\n- and again");
  });

  it("leaves a paragraph alone when a URL could pass for its words", () => {
    // The full stop ending the sentence also appears inside the link's URL, so
    // there are two readings of where it is written and only one of them is
    // right. Refusing keeps a stray keystroke out of somebody's URL; the
    // Markdown view is still there.
    const body = "See [the docs](https://example.test/docs).";
    const { document } = page({
      body,
      html: block(0, '<p>See <a href="https://example.test/docs">the docs</a>.</p>'),
      blocks: [{ span: { start: 0, end: body.length } }],
    });

    expect(document.querySelector("[contenteditable]")).toBeNull();
  });

  it("leaves a fence alone, because it has no line of prose in it", () => {
    const body = "```rust\nfn main() {}\n```";
    const { document } = page({
      body,
      html: block(0, "<pre><code>fn main() {}\n</code></pre>"),
      blocks: [{ span: { start: 0, end: body.length } }],
    });

    expect(document.querySelector("[contenteditable]")).toBeNull();
  });

  it("offers nothing on a block the pipeline reported no span for", () => {
    // The page was rendered from a deck the editor has since re-read. Offering
    // an edit against a block that is no longer in the list would splice bytes
    // belonging to another block.
    const { document } = page({ ...HEADING_AND_PROSE, blocks: [] });

    expect(document.querySelector("[contenteditable]")).toBeNull();
  });
});

describe("what changed between two versions of a line", () => {
  it("is the run between the words they still share", () => {
    expect(changeBetween("One two three", "One TWO three")).toEqual({
      from: 4,
      to: 7,
      text: "TWO",
    });
  });

  it("is nothing at all when they are the same", () => {
    expect(changeBetween("One", "One")).toBeUndefined();
  });

  it("is an insertion when nothing was taken away", () => {
    expect(changeBetween("Ready.", "Ready now.")).toEqual({ from: 5, to: 5, text: " now" });
  });

  it("is a deletion when nothing was put in", () => {
    expect(changeBetween("Ready now.", "Ready.")).toEqual({ from: 5, to: 9, text: "" });
  });

  it("never cuts an emoji in half", () => {
    // A pair of code units is one character somebody typed. Half of one is not
    // text, and the byte range would land inside a character — which the
    // pipeline refuses, so the edit would silently do nothing.
    const change = changeBetween("a🎉b", "a🎊b")!;

    expect("a🎉b".slice(change.from, change.to)).toBe("🎉");
    expect(change.text).toBe("🎊");
  });
});

describe("where a line's text is written", () => {
  const body = "A [word]{#k} here";
  const spans: BlockSpans = {
    span: { start: 0, end: body.length },
    marks: [{ span: span(body, "[word]{#k}"), words: span(body, "word") }],
  };

  function planned(html: string) {
    const page = window.document.implementation.createHTMLDocument();
    page.body.innerHTML = `<div>${html}</div>`;
    const line = page.querySelector("p")!;

    return planBlock(body, spans, editableIn(line.parentElement!)).get(line);
  }

  it("covers a mark's words as one run of their own", () => {
    const plan = planned('<p>A <span data-slidx-mark="k">word</span> here</p>')!;

    expect(plan.runs.map((run) => run.text)).toEqual(["A ", "word", " here"]);
    expect(plan.runs[1]!.source).toEqual(span(body, "word"));
  });

  it("says nothing about a line whose words are not in the source", () => {
    // A reference, a footnote, a typographic substitution. Nothing here
    // guesses: the line is not offered for editing.
    expect(planned("<p>Words from somewhere else</p>")).toBeUndefined();
  });

  it("puts a caret at the end of a mark's words inside them", () => {
    // Which is what makes typing on after a marked phrase keep the mark, rather
    // than leaving the new letters outside the class the theme styles.
    const plan = planned('<p>A <span data-slidx-mark="k">word</span> here</p>')!;
    const end = span(body, "word").end;

    expect(rangeOf(plan, { from: 6, to: 6, text: "s" })).toEqual({ start: end, end });
  });
});
