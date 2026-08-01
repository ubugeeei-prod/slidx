/**
 * The editor as a whole: four surfaces and a strip, reading one state.
 *
 * The point of these is the wiring — that a click on the outline reaches the
 * canvas, that a keystroke reaches the undo stack, and that the whole thing is
 * built out of the DOM and nothing else.
 */

import { afterEach, describe, expect, it } from "vite-plus/test";

import { deliveryRoutes, mount } from "../src/index";
import { deckOf, fakeServer } from "./support";

let mounted: ReturnType<typeof mount> | undefined;

afterEach(() => {
  mounted?.destroy();
  mounted = undefined;
  document.body.replaceChildren();
});

/**
 * Mounts into an element that is not in the page.
 *
 * The canvas is an iframe on the deck's own route, and connecting it to a
 * document would send it looking for a dev server that is not running here.
 * What these tests can check is the address it was sent to, which needs no
 * page at all.
 */
function open(server = fakeServer()) {
  const root = document.createElement("div");
  mounted = mount(root, { client: server, deckBase: "slides" });

  return { root, server };
}

/** Lets the session's first read land. */
const settled = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Chooses one narrative starting point from the outline's add-slide menu. */
function addSlide(root: HTMLElement, kind = "title-body"): void {
  root.querySelector<HTMLElement>(".slidx-slide-add-toggle")!.click();
  root.querySelector<HTMLElement>(`[data-slide-kind="${kind}"]`)!.click();
}

/**
 * Mounts with a stream that says one guest is here, on slide three.
 *
 * The stream is stubbed at `fetch` rather than injected, because the seam
 * between presence and the rest of the editor is one line inside `mount` and
 * a test that reached past it would be a test of the two halves again.
 */
async function withRoster(check: (root: HTMLElement) => Promise<void>): Promise<void> {
  const real = globalThis.fetch;
  const encoder = new TextEncoder();
  const queue = [
    'event: hello\ndata: {"id":"seat-1"}\n\n',
    `event: presence\ndata: ${JSON.stringify({
      viewers: [
        { id: "seat-1", label: "you", local: true, canEdit: true, slide: 0 },
        { id: "seat-2", label: "guest 2", local: false, canEdit: true, slide: 2, block: 1 },
      ],
    })}\n\n`,
  ];

  globalThis.fetch = ((url: string) => {
    if (!String(url).endsWith("live")) return Promise.resolve({ body: undefined });

    let at = 0;
    return Promise.resolve({
      body: {
        getReader: () => ({
          read: () =>
            Promise.resolve(
              at < queue.length
                ? { value: encoder.encode(queue[at++]!), done: false }
                : { value: undefined, done: true },
            ),
        }),
      },
    });
  }) as unknown as typeof globalThis.fetch;

  try {
    const { root } = open();
    await settled();
    for (let turn = 0; turn < queue.length + 6; turn += 1) await Promise.resolve();

    await check(root);
  } finally {
    globalThis.fetch = real;
  }
}

describe("the mounted editor", () => {
  it("derives every delivery surface from one route contract", () => {
    expect(deliveryRoutes("slides", 2)).toEqual({
      audience: "/slides/3/",
      presenter: "/slides/3/presenter/",
      print: "/slides/print/",
    });
    expect(deliveryRoutes("", 0)).toEqual({
      audience: "/",
      presenter: "/presenter/",
      print: "/print/",
    });
  });

  it("is four surfaces and a strip, and no framework under any of them", async () => {
    const { root } = open();
    await settled();

    expect(root.querySelector(".slidx-appbar")).not.toBeNull();
    expect(root.querySelector(".slidx-outline")).not.toBeNull();
    expect(root.querySelector(".slidx-slide-add-toggle")?.textContent).toContain("Add slide");
    expect(root.querySelector(".slidx-canvas")).not.toBeNull();
    expect(root.querySelector(".slidx-content-toggle")?.textContent).toContain("Add");
    expect(root.querySelector(".slidx-inspector")).not.toBeNull();
    expect(root.querySelector(".slidx-timeline")).not.toBeNull();
    expect(root.querySelector(".slidx-diagnostics")).not.toBeNull();
    expect(root.querySelector(".slidx-media-drop")).not.toBeNull();
    expect(root.querySelectorAll('[role="separator"][aria-orientation="vertical"]')).toHaveLength(
      2,
    );
  });

  it("puts the product signature and current deck above every surface", async () => {
    const { root } = open();
    await settled();

    const appbar = root.querySelector(".slidx-appbar")!;

    expect(appbar.getAttribute("aria-label")).toBe("slidx editor");
    expect(appbar.querySelectorAll(".slidx-appbar-mark rect")).toHaveLength(4);
    expect(appbar.querySelector(".slidx-appbar-title")!.textContent).toBe("A Deck");
    expect(appbar.querySelector(".slidx-appbar-position")!.textContent).toBe("1 / 3");

    root.querySelectorAll<HTMLElement>(".slidx-outline-open")[2]!.click();

    expect(appbar.querySelector(".slidx-appbar-position")!.textContent).toBe("3 / 3");
  });

  it("puts undo, redo, save state, and presentation in the shared command bar", async () => {
    const { root, server } = open();
    await settled();

    const undo = root.querySelector<HTMLButtonElement>('[aria-label="Undo"]')!;
    const redo = root.querySelector<HTMLButtonElement>('[aria-label="Redo"]')!;
    const present = root.querySelector<HTMLButtonElement>('[aria-label="Open presenter view"]')!;
    const status = root.querySelector<HTMLElement>(".slidx-appbar-status")!;

    expect(status.textContent).toBe("Saved");
    expect(undo.disabled).toBe(true);
    expect(redo.disabled).toBe(true);
    expect(present.disabled).toBe(false);

    await mounted!.session.run({ op: "setHeading", slide: 0, text: "Retitled" });
    expect(undo.disabled).toBe(false);

    undo.click();
    await settled();
    expect(server.reverted).toEqual([[{ splice: 1 }]]);
    expect(undo.disabled).toBe(true);
    expect(redo.disabled).toBe(false);
  });

  it("refreshes rendered inspector facts after the live canvas swap lands", async () => {
    const deck = deckOf("One");
    deck.spans[0]!.blocks = [{ span: { start: 0, end: 5 } }];
    const { root } = open(fakeServer(deck));
    await settled();

    const frame = root.querySelector<HTMLIFrameElement>(".slidx-canvas-frame")!;
    frame.removeAttribute("src");
    document.body.append(root);
    const block = frame.contentDocument!.createElement("div");
    block.setAttribute("data-slidx-block", "0");
    frame.contentDocument!.body.append(block);
    mounted!.session.select({ block: 0 });

    const state = () => root.querySelector(".slidx-frame-position-state")!.textContent;
    expect(state()).toBe("Following layout");

    block.setAttribute("data-slidx-freeform-frame", "");
    frame.dispatchEvent(new Event("load"));

    expect(state()).toBe("Pinned to safe area");
    expect(root.querySelector<HTMLButtonElement>(".slidx-frame-reset")!.disabled).toBe(false);
  });

  it("turns a read capability into a review workspace before any edit is attempted", async () => {
    const deck = deckOf("One", "Two");
    deck.access = { canEdit: false };
    const server = fakeServer(deck);
    const { root } = open(server);
    await settled();

    const editor = root.querySelector<HTMLElement>(".slidx-editor")!;
    expect(editor.dataset.access).toBe("read");
    expect(root.querySelector(".slidx-appbar-status")!.textContent).toBe("View only");
    expect(root.querySelector<HTMLTextAreaElement>(".slidx-canvas-source")!.readOnly).toBe(true);

    await mounted!.session.run({ op: "setHeading", slide: 0, text: "Not sent" });
    expect(server.ops).toEqual([]);
  });

  it("searches commands and slide titles from the shared command bar", async () => {
    const { root } = open();
    await settled();

    const event = new KeyboardEvent("keydown", {
      key: "k",
      metaKey: true,
      bubbles: true,
      cancelable: true,
    });
    document.dispatchEvent(event);

    const palette = root.querySelector<HTMLElement>(".slidx-command-palette")!;
    const input = palette.querySelector<HTMLInputElement>(".slidx-command-input")!;
    input.value = "Three";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    palette.querySelector<HTMLButtonElement>('[role="option"]')!.click();

    expect(event.defaultPrevented).toBe(true);
    expect(mounted!.session.state().selection.slide).toBe(2);
    expect(root.querySelector(".slidx-canvas-frame")!.getAttribute("src")).toMatch(
      /^\/slides\/3\//,
    );
  });

  it("gives the canvas the workspace and restores every panel without losing state", async () => {
    const { root } = open();
    await settled();
    const editor = root.querySelector<HTMLElement>(".slidx-editor")!;
    const focus = root.querySelector<HTMLButtonElement>('[aria-label="Focus canvas"]')!;

    focus.click();

    expect(editor.getAttribute("data-canvas-focus")).toBe("true");
    expect(focus.getAttribute("aria-pressed")).toBe("true");
    expect(focus.getAttribute("aria-label")).toBe("Restore workspace");
    expect(root.querySelector(".slidx-outline")).not.toBeNull();
    expect(root.querySelector(".slidx-inspector")).not.toBeNull();

    document.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
    );

    expect(editor.getAttribute("data-canvas-focus")).toBe("false");
    expect(focus.getAttribute("aria-pressed")).toBe("false");
    expect(mounted!.session.state().selection.slide).toBe(0);
  });

  it("draws the timeline for the slide being edited, from the deck it read", async () => {
    // The reason this is a mount test rather than a surface one: a panel that
    // works when handed a grid and is never handed one is a panel nobody can
    // reach, which is the failure this repository writes a roadmap rule about.
    const deck = deckOf("One", "Two");
    deck.deck.slides[1]!.stopCount = 3;
    deck.deck.slides[1]!.steps = {
      declared: true,
      stops: 3,
      rows: [
        { target: "#a", label: "first", key: "a", visible: [false, true, true] },
        { target: "#b", label: "second", key: "b", visible: [false, false, true] },
      ],
      actions: [
        {
          index: 0,
          kind: "reveal",
          stop: 1,
          targets: ["#a"],
          timed: false,
          source: 'reveal: "#a"',
        },
        {
          index: 1,
          kind: "reveal",
          stop: 2,
          targets: ["#b"],
          timed: false,
          source: 'reveal: "#b"',
        },
      ],
    };

    const { root, server } = open(fakeServer(deck));
    await settled();

    root.querySelectorAll<HTMLElement>(".slidx-outline-open")[1]!.click();

    expect(root.querySelectorAll(".slidx-timeline-row")).toHaveLength(2);
    expect(root.querySelector(".slidx-timeline-where")!.textContent).toContain("0 of 3");

    root
      .querySelectorAll(".slidx-timeline-row")[1]!
      .querySelectorAll<HTMLElement>(".slidx-timeline-cell")[2]!
      .click();
    await settled();

    expect(server.ops).toEqual([{ op: "removeStep", slide: 1, index: 1 }]);
  });

  it("carries its own chrome rather than asking a project for a stylesheet", async () => {
    open();
    await settled();

    expect(document.querySelector("style[data-slidx-editor]")).not.toBeNull();
  });

  it("shows the deck it read", async () => {
    const { root } = open();
    await settled();

    expect(root.querySelectorAll(".slidx-outline-row")).toHaveLength(3);
    expect(
      [...root.querySelectorAll<HTMLIFrameElement>(".slidx-outline-frame")].map(
        (frame) => frame.dataset.preview,
      ),
    ).toEqual(["/slides/", "/slides/2/", "/slides/3/"]);
  });

  it("keeps detached outline previews inert while retaining their routes", async () => {
    const { root } = open();
    await settled();
    const frames = [...root.querySelectorAll<HTMLIFrameElement>(".slidx-outline-frame")];

    expect(frames.map((frame) => frame.dataset.preview)).toEqual([
      "/slides/",
      "/slides/2/",
      "/slides/3/",
    ]);
    expect(frames.map((frame) => frame.getAttribute("src"))).toEqual([null, null, null]);

    mounted!.session.select({ slide: 2 });
    mounted!.session.saw([{ id: "seat-2", label: "guest", slide: 1, local: false, canEdit: true }]);

    expect([...root.querySelectorAll<HTMLIFrameElement>(".slidx-outline-frame")]).toEqual(frames);
    expect(frames.map((frame) => frame.dataset.preview)).toEqual([
      "/slides/",
      "/slides/2/",
      "/slides/3/",
    ]);
    expect(frames.map((frame) => frame.getAttribute("src"))).toEqual([null, null, null]);
  });

  it("points the canvas at the deck's own page for the slide being edited", async () => {
    const { root } = open();
    await settled();

    root.querySelectorAll<HTMLElement>(".slidx-outline-open")[2]!.click();

    const frame = root.querySelector<HTMLIFrameElement>(".slidx-canvas-frame")!;
    expect(frame.getAttribute("src")).toMatch(/^\/slides\/3\/\?/);
  });

  it("selects and quietly outlines a block in the rendered slide", async () => {
    const deck = deckOf("One");
    deck.spans[0]!.blocks = [{ span: { start: 0, end: 5 } }];
    const { root } = open(fakeServer(deck));
    await settled();

    const frame = root.querySelector<HTMLIFrameElement>(".slidx-canvas-frame")!;
    frame.removeAttribute("src");
    for (const preview of root.querySelectorAll<HTMLIFrameElement>(".slidx-outline-frame")) {
      preview.removeAttribute("src");
    }
    document.body.append(root);
    frame.contentDocument!.body.innerHTML = `
      <article class="slidx-slide">
        <div class="slidx-slide-body">
          <div class="slidx-block" data-slidx-block="0"><h1>One</h1></div>
        </div>
      </article>
    `;
    frame.dispatchEvent(new Event("load"));
    frame
      .contentDocument!.querySelector("h1")!
      .dispatchEvent(new window.PointerEvent("pointerdown", { bubbles: true }));

    expect(mounted!.session.state().selection).toEqual({ slide: 0, block: 0 });
    expect(
      frame
        .contentDocument!.querySelector(".slidx-block")!
        .hasAttribute("data-slidx-editor-selected"),
    ).toBe(true);
  });

  it("adds after the current slide and opens the first draft it created", async () => {
    const server = fakeServer();
    server.answer = deckOf("One", "New slide", "Two", "Three");
    const { root } = open(server);
    await settled();

    addSlide(root, "comparison");
    await settled();

    expect(server.ops).toEqual([{ op: "createSlide", at: 1, kind: "comparison" }]);
    expect(mounted!.session.state().selection.slide).toBe(1);
  });

  it("creates a searched slide composition and opens it for editing", async () => {
    const server = fakeServer();
    server.answer = deckOf("One", "New slide", "Two", "Three");
    const { root } = open(server);
    await settled();

    root.querySelector<HTMLButtonElement>(".slidx-command-trigger")!.click();
    const search = root.querySelector<HTMLInputElement>(".slidx-command-input")!;
    search.value = "comparison slide";
    search.dispatchEvent(new Event("input", { bubbles: true }));
    root.querySelector<HTMLButtonElement>('[role="option"][aria-selected="true"]')!.click();
    await settled();

    expect(server.ops).toEqual([{ op: "createSlide", at: 1, kind: "comparison" }]);
    expect(mounted!.session.state().selection.slide).toBe(1);
  });

  it("undoes on the shortcut every editor on both platforms uses", async () => {
    const { root, server } = open();
    await settled();

    addSlide(root);
    await settled();

    document.dispatchEvent(new KeyboardEvent("keydown", { key: "z", metaKey: true }));
    await settled();

    expect(server.ops).toHaveLength(1);
    expect(server.reverted).toHaveLength(1);
  });

  it("redoes on the same shortcut with shift", async () => {
    const { root, server } = open();
    await settled();

    addSlide(root);
    await settled();
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "z", metaKey: true }));
    await settled();
    document.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Z", metaKey: true, shiftKey: true }),
    );
    await settled();

    expect(server.reverted).toHaveLength(2);
  });

  it("copies a slide, pastes it elsewhere, and selects the new copy", async () => {
    const server = fakeServer();
    const { root } = open(server);
    await settled();
    root.querySelectorAll<HTMLElement>(".slidx-outline-open")[1]!.click();
    const data = new DataTransfer();

    document.dispatchEvent(new ClipboardEvent("copy", { cancelable: true, clipboardData: data }));
    root.querySelectorAll<HTMLElement>(".slidx-outline-open")[2]!.click();
    server.answer = deckOf("One", "Two", "Three", "Two copy");
    document.dispatchEvent(new ClipboardEvent("paste", { cancelable: true, clipboardData: data }));
    await settled();

    expect(server.ops).toEqual([{ op: "duplicateSlide", slide: 1, after: 2 }]);
    expect(mounted!.session.state().selection).toEqual({ slide: 3 });
  });

  it("stops listening once it is taken down", async () => {
    const { root, server } = open();
    await settled();
    addSlide(root);
    await settled();

    mounted!.destroy();
    mounted = undefined;
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "z", metaKey: true }));
    await settled();

    expect(server.reverted).toHaveLength(0);
  });

  it("opens the storyboard onto the deck and the slot it was given", async () => {
    // The reachable path, which is the only thing that makes the feature real:
    // one control in the editor a dev server serves, and behind it the deck laid
    // against the `duration:` the session carried through from the pipeline.
    const { root } = open();
    await settled();

    root.querySelector<HTMLElement>(".slidx-sb-launch")!.click();

    expect(root.querySelectorAll(".slidx-sb-slide")).toHaveLength(3);
    expect(root.querySelector(".slidx-sb-summary")!.textContent).toContain("a 10m slot");
  });

  it("says what the linter found without asking for it again", async () => {
    // The pipeline returns findings with every parse, so showing them inline
    // costs a read rather than a second analysis.
    const deck = deckOf("One", "Two");
    deck.deck.diagnostics = [
      { severity: "warning", code: "a11y/alt", message: "no alt text", slideIndex: 1 },
    ];

    const { root } = open(fakeServer(deck));
    await settled();

    expect(root.querySelector(".slidx-diagnostics")!.textContent).toContain("no alt text");
    expect(root.querySelectorAll(".slidx-outline-row")[1]!.getAttribute("data-severity")).toBe(
      "warning",
    );
  });

  it("puts the marks for the other editors over the canvas, and outside it", async () => {
    // The same rule the arrange overlay follows, for the same reason: a mark
    // drawn inside the frame would be an element the build never emits, on the
    // page whose whole job is to be the page the build emits.
    const { root } = open();
    await settled();

    const overlay = root.querySelector(".slidx-beacons");
    expect(overlay).not.toBeNull();
    expect(overlay!.closest(".slidx-canvas")).toBeNull();
  });

  it("docks collaboration in the shared command bar instead of over canvas tools", async () => {
    const { root } = open();
    await settled();

    const presence = root.querySelector(".slidx-presence");
    expect(presence).not.toBeNull();
    expect(presence!.closest(".slidx-appbar")).not.toBeNull();
    expect(presence!.closest(".slidx-canvas")).toBeNull();
  });

  it("puts a roster arriving on the stream into the state every surface reads", async () => {
    // The seam that would otherwise be a line nothing runs. Presence is the
    // only thing in the editor that reads a stream, and the marks on the canvas
    // are drawn from state — so if this one call is missing, both surfaces work
    // in isolation and nobody ever sees anyone.
    await withRoster(async () => {
      expect(mounted!.session.state().viewers.map((viewer) => viewer.label)).toEqual([
        "you",
        "guest 2",
      ]);
    });
  });

  it("goes to the slide a guest is on when their row is pressed", async () => {
    // The whole path, from a frame on the stream to the address the canvas is
    // pointed at: the roster is drawn, the row is a control, pressing it
    // follows that seat, and following one moves this editor to their slide.
    await withRoster(async (root) => {
      const rows = root.querySelectorAll<HTMLButtonElement>("button.slidx-presence-seat");
      expect(rows).toHaveLength(1);

      rows[0]!.click();

      expect(mounted!.session.state().following).toBe("seat-2");
      expect(mounted!.session.state().selection.slide).toBe(2);
      expect(
        root.querySelector<HTMLIFrameElement>(".slidx-canvas-frame")!.getAttribute("src"),
      ).toMatch(/^\/slides\/3\//);
    });
  });

  it("stops following the moment the author picks a slide themselves", async () => {
    await withRoster(async (root) => {
      root.querySelector<HTMLButtonElement>("button.slidx-presence-seat")!.click();
      root.querySelectorAll<HTMLElement>(".slidx-outline-open")[0]!.click();

      expect(mounted!.session.state().following).toBeUndefined();
      expect(mounted!.session.state().selection.slide).toBe(0);
    });
  });

  it("puts the arrange overlay over the canvas rather than inside it", async () => {
    // A surface that works when handed a geometry and is never handed one is a
    // surface nobody can reach — and the overlay has to be on this side of the
    // frame, because the deck page is the page the build emits.
    const { root } = open();
    await settled();

    const overlay = root.querySelector(".slidx-arrange");
    expect(overlay).not.toBeNull();
    expect(overlay!.closest(".slidx-canvas")).toBeNull();
  });
});
