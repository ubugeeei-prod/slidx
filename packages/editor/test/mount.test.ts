/**
 * The editor as a whole: four surfaces and a strip, reading one state.
 *
 * The point of these is the wiring — that a click on the outline reaches the
 * canvas, that a keystroke reaches the undo stack, and that the whole thing is
 * built out of the DOM and nothing else.
 */

import { afterEach, describe, expect, it } from "vite-plus/test";

import { mount } from "../src/index";
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

describe("the mounted editor", () => {
  it("is four surfaces and a strip, and no framework under any of them", async () => {
    const { root } = open();
    await settled();

    expect(root.querySelector(".slidx-outline")).not.toBeNull();
    expect(root.querySelector(".slidx-canvas")).not.toBeNull();
    expect(root.querySelector(".slidx-inspector")).not.toBeNull();
    expect(root.querySelector(".slidx-timeline")).not.toBeNull();
    expect(root.querySelector(".slidx-diagnostics")).not.toBeNull();
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
        { index: 0, kind: "reveal", stop: 1, targets: ["#a"], timed: false, source: 'reveal: "#a"' },
        { index: 1, kind: "reveal", stop: 2, targets: ["#b"], timed: false, source: 'reveal: "#b"' },
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
  });

  it("points the canvas at the deck's own page for the slide being edited", async () => {
    const { root } = open();
    await settled();

    root.querySelectorAll<HTMLElement>(".slidx-outline-open")[2]!.click();

    const frame = root.querySelector<HTMLIFrameElement>(".slidx-canvas-frame")!;
    expect(frame.getAttribute("src")).toMatch(/^\/slides\/3\/\?/);
  });

  it("undoes on the shortcut every editor on both platforms uses", async () => {
    const { root, server } = open();
    await settled();

    root.querySelector<HTMLElement>(".slidx-add")!.click();
    await settled();

    document.dispatchEvent(new KeyboardEvent("keydown", { key: "z", metaKey: true }));
    await settled();

    expect(server.ops).toHaveLength(1);
    expect(server.reverted).toHaveLength(1);
  });

  it("redoes on the same shortcut with shift", async () => {
    const { root, server } = open();
    await settled();

    root.querySelector<HTMLElement>(".slidx-add")!.click();
    await settled();
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "z", metaKey: true }));
    await settled();
    document.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Z", metaKey: true, shiftKey: true }),
    );
    await settled();

    expect(server.reverted).toHaveLength(2);
  });

  it("stops listening once it is taken down", async () => {
    const { root, server } = open();
    await settled();
    root.querySelector<HTMLElement>(".slidx-add")!.click();
    await settled();

    mounted!.destroy();
    mounted = undefined;
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "z", metaKey: true }));
    await settled();

    expect(server.reverted).toHaveLength(0);
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
});
