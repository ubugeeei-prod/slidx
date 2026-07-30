/**
 * A change reaching the canvas without the page under it being thrown away.
 *
 * What these are about is the *document*: an iframe that reloads loses the
 * caret, the selection and the scroll position, so every test here asserts that
 * the document the frame started with is the document it ends with — and that
 * the markup inside it is what the route serves, because a preview drawn any
 * other way would be a second answer about layout.
 */

import { afterEach, describe, expect, it } from "vite-plus/test";

import { createCanvas } from "../src/canvas";
import { canPatch, caretIn, patch, restore } from "../src/live";
import type { EditorState } from "../src/session";

/** A frame holding a slide, the way the deck's route serves one. */
function canvasFrame(inner: string): HTMLIFrameElement {
  const frame = document.createElement("iframe");
  document.body.append(frame);
  frame.contentDocument!.body.innerHTML = inner;

  return frame;
}

const SLIDE = `
<article class="slidx-slide">
  <div class="slidx-slide-body">
    <div class="slidx-region" data-slidx-region="body">
      <div class="slidx-block" data-slidx-block="0"><h1>One</h1></div>
      <div class="slidx-block" data-slidx-block="1"><p>Second.</p></div>
    </div>
  </div>
</article>`;

/** A server that answers one page, and counts how often it was asked. */
function serving(html: string) {
  const asked: string[] = [];

  const send = (async (url: string) => {
    asked.push(String(url));
    return { ok: true, text: async () => `<!doctype html><body>${html}</body>` };
  }) as unknown as typeof globalThis.fetch;

  return { asked, send };
}

function stateOf(over: Partial<EditorState> = {}): EditorState {
  return {
    source: "# One\n\nSecond.",
    spans: [{ content: { start: 0, end: 15 }, body: { start: 0, end: 15 } }],
    slides: [
      {
        id: "one",
        index: 0,
        title: "One",
        notes: [],
        stopCount: 1,
        estimatedSeconds: 0,
        optional: false,
      },
    ],
    diagnostics: [],
    selection: { slide: 0 },
    canUndo: false,
    canRedo: false,
    ...over,
  };
}

/**
 * Waits for the round trip to the route to finish.
 *
 * Bringing the frame up to date is a fetch, so it settles a task later rather
 * than a microtask later — and a test that only awaited a microtask would pass
 * while nothing had happened yet.
 */
const settled = () => new Promise((resolve) => setTimeout(resolve, 0));

afterEach(() => document.body.replaceChildren());

describe("bringing the canvas up to date", () => {
  it("replaces the slide with the one the route serves and keeps the document", async () => {
    const frame = canvasFrame(SLIDE);
    const before = frame.contentDocument;
    const server = serving(SLIDE.replace("Second.", "Third."));

    expect(await patch(frame, "/slides/", server.send)).toBe(true);

    expect(frame.contentDocument).toBe(before);
    expect(frame.contentDocument!.body.textContent).toContain("Third.");
    expect(server.asked).toEqual(["/slides/"]);
  });

  it("says no when the page it was given is not a slide", async () => {
    // A route that answered with something else, or a dev server that has
    // stopped. Reloading is always correct; leaving the canvas showing the deck
    // as it was a keystroke ago is not.
    const frame = canvasFrame(SLIDE);

    expect(await patch(frame, "/slides/", serving("<p>Not a deck.</p>").send)).toBe(false);
  });

  it("says no when the request fails at all", async () => {
    const frame = canvasFrame(SLIDE);
    const send = (async () => {
      throw new Error("the dev server went away");
    }) as unknown as typeof globalThis.fetch;

    expect(await patch(frame, "/slides/", send)).toBe(false);
  });

  it("refuses a slide with steps, whose staging is bound to the elements", () => {
    // Anchors are resolved by mutating the DOM, so a stage cannot be re-bound
    // and a swapped slide would show every stop at once.
    const frame = canvasFrame(SLIDE);

    expect(canPatch(frame, 1)).toBe(true);
    expect(canPatch(frame, 4)).toBe(false);
  });
});

describe("the caret, across a change", () => {
  it("is remembered as a block and a position rather than as an element", () => {
    // The element is about to be replaced by one the server rendered, so an
    // element is exactly what cannot be held on to.
    const frame = canvasFrame(SLIDE);
    const page = frame.contentDocument!;
    const line = page.querySelector("p")!;
    line.setAttribute("contenteditable", "true");
    line.focus();

    const range = page.createRange();
    range.setStart(line.firstChild!, 3);
    page.getSelection()!.removeAllRanges();
    page.getSelection()!.addRange(range);

    expect(caretIn(page)).toEqual({ block: 1, line: 0, at: 3 });
  });

  it("is nothing at all when the author was not in a line", () => {
    const frame = canvasFrame(SLIDE);

    expect(caretIn(frame.contentDocument!)).toBeUndefined();
  });

  it("goes back into the words rather than to the front of the line", () => {
    // A `contenteditable` focused with no range puts the caret at its start,
    // which is a smaller version of the same complaint.
    const frame = canvasFrame(SLIDE);
    const page = frame.contentDocument!;
    const line = page.querySelector("p")!;
    line.setAttribute("contenteditable", "true");

    restore(page, line, 4);

    const selection = page.getSelection()!;
    expect(selection.rangeCount).toBe(1);
    expect(selection.getRangeAt(0).startOffset).toBe(4);
  });
});

describe("the canvas, when the deck changes", () => {
  it("does not reload the frame for a change on the slide being edited", async () => {
    const server = serving(SLIDE.replace("Second.", "Third."));
    const canvas = createCanvas(
      { run: () => {}, selected: () => {} },
      { deckBase: "slides", bodyOf: () => "# One\n\nSecond.", fetch: server.send },
    );
    document.body.append(canvas.root);

    const frame = canvas.root.querySelector<HTMLIFrameElement>(".slidx-canvas-frame")!;
    canvas.render(stateOf());
    // The first render has nothing to patch: the frame has not loaded a slide.
    expect(frame.getAttribute("src")).toContain("/slides/");

    frame.contentDocument!.body.innerHTML = SLIDE;
    const was = frame.getAttribute("src");

    canvas.render(stateOf({ source: "# One\n\nThird." }));
    await settled();

    expect(frame.getAttribute("src")).toBe(was);
    expect(server.asked).toEqual(["/slides/"]);
    expect(frame.contentDocument!.body.textContent).toContain("Third.");
  });

  it("tells the frame it changed, so what measures inside it measures again", async () => {
    // The lines that become editable, the grips the arrange overlay draws and
    // the handles the resize overlay draws all wait for a load, and a slide
    // replaced in place never loads. Asserted through the lines because they
    // are the consequence this file can see: a `contenteditable` on markup that
    // arrived after the swap could only have been put there by that signal.
    let body = "# One\n\nSecond.";
    const server = serving(SLIDE.replace("Second.", "Third."));
    const canvas = createCanvas(
      { run: () => {}, selected: () => {} },
      {
        deckBase: "slides",
        bodyOf: () => body,
        blocksOf: () => [{ span: { start: 0, end: 5 } }, { span: { start: 7, end: 13 } }],
        fetch: server.send,
      },
    );
    document.body.append(canvas.root);

    const frame = canvas.root.querySelector<HTMLIFrameElement>(".slidx-canvas-frame")!;
    canvas.render(stateOf());
    frame.contentDocument!.body.innerHTML = SLIDE;

    body = "# One\n\nThird.";
    canvas.render(stateOf({ source: body }));
    await settled();

    const line = frame.contentDocument!.querySelector("p")!;
    expect(line.textContent).toBe("Third.");
    expect(line.getAttribute("contenteditable")).toBe("true");
  });

  it("reloads the frame when the author moved to another slide", async () => {
    // A different page, which is a navigation rather than a change.
    const server = serving(SLIDE);
    const canvas = createCanvas(
      { run: () => {}, selected: () => {} },
      { deckBase: "slides", bodyOf: () => "# One", fetch: server.send },
    );
    document.body.append(canvas.root);

    const frame = canvas.root.querySelector<HTMLIFrameElement>(".slidx-canvas-frame")!;
    canvas.render(stateOf());
    frame.contentDocument!.body.innerHTML = SLIDE;

    canvas.render(stateOf({ selection: { slide: 1 } }));
    await settled();

    expect(frame.getAttribute("src")).toContain("/slides/2/");
    expect(server.asked).toEqual([]);
  });

  it("reloads the frame for a slide with steps", async () => {
    const server = serving(SLIDE);
    const canvas = createCanvas(
      { run: () => {}, selected: () => {} },
      { deckBase: "slides", bodyOf: () => "# One", fetch: server.send },
    );
    document.body.append(canvas.root);

    const frame = canvas.root.querySelector<HTMLIFrameElement>(".slidx-canvas-frame")!;
    const staged = stateOf();
    staged.slides[0]!.stopCount = 3;

    canvas.render(staged);
    frame.contentDocument!.body.innerHTML = SLIDE;
    const was = frame.getAttribute("src");

    canvas.render({ ...staged, source: "# One\n\nChanged." });
    await settled();

    expect(frame.getAttribute("src")).not.toBe(was);
    expect(server.asked).toEqual([]);
  });
});
