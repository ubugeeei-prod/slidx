/**
 * External files crossing the editor and becoming semantic media operations.
 *
 * Geometry and bounds are values here because DOM test environments do not lay
 * out an iframe. The surface still receives real drag events and real Files,
 * which is where filtering, ordering and failure cleanup tend to break.
 */

import { afterEach, describe, expect, it } from "vite-plus/test";

import type { EditorState } from "../src/session";
import type { SlideGeometry } from "../src/geometry";
import { createMediaDrop } from "../src/media-drop";
import { MEDIA_DROP_STYLESHEET } from "../src/media-drop-styles";
import type { EditOp } from "../src/operations";

const geometry: SlideGeometry = {
  slide: { left: 80, top: 80, width: 840, height: 640 },
  safe: { left: 100, top: 100, width: 800, height: 600 },
  regions: [
    {
      name: "main",
      rect: { left: 100, top: 100, width: 500, height: 600 },
      blocks: [0],
      contentHeight: 40,
      gap: 20,
    },
    {
      name: "side",
      rect: { left: 620, top: 100, width: 280, height: 600 },
      blocks: [],
      contentHeight: 0,
      gap: 20,
    },
  ],
  blocks: [
    {
      index: 0,
      region: "main",
      rect: { left: 100, top: 140, width: 500, height: 40 },
      needsWidth: 0,
      width: "full",
    },
  ],
};

let surface: ReturnType<typeof createMediaDrop> | undefined;

afterEach(() => {
  surface?.destroy?.();
  surface = undefined;
  document.body.replaceChildren();
});

function drag(type: string, files: File[], x: number, y: number): DragEvent {
  const event = new Event(type, { bubbles: true, cancelable: true }) as DragEvent;
  const transfer = { types: ["Files"], files, dropEffect: "none" };
  Object.defineProperties(event, {
    clientX: { value: x },
    clientY: { value: y },
    dataTransfer: { value: transfer },
    relatedTarget: { value: document.body },
  });
  return event;
}

const settled = () => new Promise((resolve) => setTimeout(resolve, 0));

function open(
  over: {
    upload?(file: File): Promise<{ kind: "image" | "video"; src: string; alt: string }>;
    run?(op: EditOp): void | Promise<void>;
  } = {},
) {
  const uploaded: File[] = [];
  const ops: EditOp[] = [];
  surface = createMediaDrop(
    {
      async upload(file) {
        uploaded.push(file);
        if (over.upload) return over.upload(file);
        return {
          kind: file.type.startsWith("video/") ? "video" : "image",
          src: `/slides/assets/${file.name}`,
          alt: file.name.replace(/\.[^.]+$/, ""),
        };
      },
      async run(op) {
        ops.push(op);
        await over.run?.(op);
      },
    },
    {
      geometry: () => geometry,
      bounds: () => ({ left: 60, top: 60, width: 860, height: 680 }),
    },
  );
  document.body.append(surface.root);
  surface.render({ selection: { slide: 2 } } as EditorState);

  return { root: surface.root, uploaded, ops };
}

describe("media drag and drop", () => {
  it("shows the region and insertion line under image files", () => {
    const { root } = open();
    const file = new File(["png"], "chart.png", { type: "image/png" });

    document.dispatchEvent(drag("dragenter", [file], 120, 110));
    document.dispatchEvent(drag("dragover", [file], 120, 110));

    expect(root.getAttribute("data-active")).toBe("true");
    expect(root.getAttribute("data-target")).toBe("main");
    expect(root.querySelector<HTMLElement>(".slidx-media-drop-region")!.style.width).toBe("500px");
    expect(root.querySelector<HTMLElement>(".slidx-media-drop-line")!.style.height).toBe("2px");
  });

  it("translates a drop inside the preview frame into editor coordinates", async () => {
    const frame = document.createElement("iframe");
    frame.className = "slidx-canvas-frame";
    frame.getBoundingClientRect = () =>
      ({
        left: 100,
        top: 90,
        width: 800,
        height: 600,
      }) as DOMRect;
    Object.defineProperty(frame, "clientLeft", { value: 2 });
    Object.defineProperty(frame, "clientTop", { value: 2 });
    document.body.append(frame);
    const { uploaded, ops } = open();
    const image = new File(["png"], "frame.png", { type: "image/png" });

    frame.contentDocument!.dispatchEvent(drag("drop", [image], 20, 20));
    await settled();

    expect(uploaded).toEqual([image]);
    expect(ops).toMatchObject([
      { op: "insertMedia", slide: 2, at: 0, region: "main", kind: "image" },
    ]);
  });

  it("uploads multiple files in order and commits semantic operations in one region", async () => {
    const { root, uploaded, ops } = open();
    const image = new File(["png"], "chart.png", { type: "image/png" });
    const video = new File(["mp4"], "demo.mp4", { type: "video/mp4" });

    document.dispatchEvent(drag("drop", [image, video], 120, 110));
    await settled();

    expect(uploaded.map((file) => file.name)).toEqual(["chart.png", "demo.mp4"]);
    expect(ops).toEqual([
      {
        op: "insertMedia",
        slide: 2,
        at: 0,
        region: "main",
        kind: "image",
        src: "/slides/assets/chart.png",
        alt: "chart",
      },
      {
        op: "insertMedia",
        slide: 2,
        at: 1,
        region: "main",
        kind: "video",
        src: "/slides/assets/demo.mp4",
        alt: "demo",
      },
    ]);
    expect(root.getAttribute("data-active")).toBe("false");
    expect(root.querySelector(".slidx-media-drop-status")!.textContent).toContain("2 media files");
  });

  it("keeps the slide the gesture started on while an upload is in flight", async () => {
    let release: (() => void) | undefined;
    const waiting = new Promise<void>((resolve) => {
      release = resolve;
    });
    const { ops } = open({
      upload: async (file) => {
        await waiting;
        return { kind: "image", src: file.name, alt: "chart" };
      },
    });
    const image = new File(["png"], "chart.png", { type: "image/png" });

    document.dispatchEvent(drag("drop", [image], 120, 110));
    surface!.render({ selection: { slide: 7 } } as EditorState);
    release!();
    await settled();

    expect(ops).toMatchObject([{ op: "insertMedia", slide: 2 }]);
  });

  it("ignores unsupported files without uploading or editing", async () => {
    const { root, uploaded, ops } = open();
    const text = new File(["notes"], "notes.txt", { type: "text/plain" });

    document.dispatchEvent(drag("drop", [text], 120, 110));
    await settled();

    expect(uploaded).toEqual([]);
    expect(ops).toEqual([]);
    expect(root.getAttribute("data-active")).toBe("false");
    expect(root.querySelector(".slidx-media-drop-status")!.textContent).toContain(
      "Only image and video",
    );
  });

  it("does not upload a media file dropped outside a layout region", async () => {
    const { uploaded, ops } = open();
    const image = new File(["png"], "chart.png", { type: "image/png" });

    document.dispatchEvent(drag("drop", [image], 1_200, 400));
    await settled();

    expect(uploaded).toEqual([]);
    expect(ops).toEqual([]);
  });

  it("keeps an upload failure visible until Escape dismisses it", async () => {
    const { root, ops } = open({
      upload: async () => {
        throw new Error("The file is too large.");
      },
    });
    const video = new File(["mp4"], "keynote.mp4", { type: "video/mp4" });

    document.dispatchEvent(drag("drop", [video], 120, 110));
    await settled();

    expect(ops).toEqual([]);
    expect(root.getAttribute("data-error")).toBe("true");
    expect(root.getAttribute("data-active")).toBe("true");
    expect(root.querySelector(".slidx-media-drop-message")!.textContent).toContain(
      "The file is too large.",
    );

    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }));
    expect(root.getAttribute("data-error")).toBe("false");
    expect(root.getAttribute("data-active")).toBe("false");
  });

  it("uses generous spacing and quiet hairlines without decorative effects", () => {
    expect(MEDIA_DROP_STYLESHEET).toContain("padding: var(--slidx-e-loose)");
    expect(MEDIA_DROP_STYLESHEET).toContain("var(--slidx-e-hairline) solid");
    expect(MEDIA_DROP_STYLESHEET).not.toMatch(/box-shadow|(?:linear|radial)-gradient/);
  });
});
