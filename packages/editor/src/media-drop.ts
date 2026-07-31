/**
 * Image and video files dropped onto the real rendered slide.
 *
 * The target lives outside the iframe, over boxes measured from the page the
 * build emits. The browser uploads bytes and then sends one semantic
 * `insertMedia` operation; it never constructs Markdown or guesses an asset
 * path. That keeps file naming, escaping, regions, collaboration and undo in
 * the same pipeline as every other visual edit.
 */

import type { UploadedMedia } from "./client";
import { element } from "./dom";
import { readGeometry, type Rect, type SlideGeometry } from "./geometry";
import type { EditOp } from "./operations";
import type { Surface } from "./outline";
import { insertion, type Insertion } from "./placement";
import { applyMediaDropStyles } from "./media-drop-styles";

export interface MediaDropHandlers {
  upload(file: File): Promise<UploadedMedia>;
  run(op: EditOp): void | Promise<void>;
}

export interface MediaDropOptions {
  /** The slide boxes. Injected so a drop is testable without browser layout. */
  geometry?(): SlideGeometry | undefined;
  /** The canvas stage. Injected for the same reason. */
  bounds?(): Rect | undefined;
}

interface Point {
  x: number;
  y: number;
}

const READY = "Drop images or videos into a layout region";
const OUTSIDE = "Move into a layout region to place these files";

export function createMediaDrop(
  handlers: MediaDropHandlers,
  options: MediaDropOptions = {},
): Surface {
  const region = element("div", { class: "slidx-media-drop-region" });
  const line = element("div", { class: "slidx-media-drop-line" });
  const message = element(
    "button",
    {
      class: "slidx-media-drop-message",
      type: "button",
      tabindex: -1,
      "aria-label": "Dismiss media drop message",
    },
    [READY],
  ) as HTMLButtonElement;
  const status = element("div", {
    class: "slidx-media-drop-status",
    role: "status",
    "aria-live": "polite",
  });
  const root = element(
    "div",
    {
      class: "slidx-media-drop",
      "data-active": "false",
      "data-target": "",
      "data-busy": "false",
      "data-error": "false",
      "aria-label": "Media drop target",
    },
    [region, line, message, status],
  );
  applyMediaDropStyles(root.ownerDocument);

  let slide = 0;
  let geometry: SlideGeometry | undefined;
  let target: Insertion | undefined;
  let frame: HTMLIFrameElement | undefined;
  let page: Document | undefined;
  let busy = false;
  let destroyed = false;

  function box(node: HTMLElement, rect: Rect): void {
    node.style.left = `${rect.left}px`;
    node.style.top = `${rect.top}px`;
    node.style.width = `${rect.width}px`;
    node.style.height = `${rect.height}px`;
  }

  function read(): SlideGeometry | undefined {
    if (options.geometry) return options.geometry();
    return frame ? readGeometry(frame) : undefined;
  }

  function stageBounds(): Rect | undefined {
    if (options.bounds) return options.bounds();
    const stage = root.ownerDocument.querySelector(".slidx-canvas-stage");
    if (!stage) return undefined;
    const rect = stage.getBoundingClientRect();
    return { left: rect.left, top: rect.top, width: rect.width, height: rect.height };
  }

  function refresh(): void {
    const bounds = stageBounds();
    if (bounds) box(root, bounds);
    geometry = read();
    if (target && !geometry?.regions.some((candidate) => candidate.name === target?.region)) {
      paint(undefined);
    }
  }

  function active(value: boolean): void {
    root.setAttribute("data-active", String(value));
    if (!value) paint(undefined);
  }

  function paint(next: Insertion | undefined): void {
    target = next;
    root.setAttribute("data-target", next?.region ?? "");
    if (!next || !geometry) {
      message.textContent = OUTSIDE;
      return;
    }

    const targetRegion = geometry.regions.find((candidate) => candidate.name === next.region);
    if (!targetRegion) return;
    box(region, targetRegion.rect);
    box(line, { ...next.rect, height: 2 });
    message.textContent = `${next.region} · position ${next.at + 1}`;
  }

  function isFileDrag(event: DragEvent): boolean {
    return Array.from(event.dataTransfer?.types ?? []).includes("Files");
  }

  function mediaFiles(event: DragEvent): File[] {
    return Array.from(event.dataTransfer?.files ?? []).filter(
      (file) => file.type.startsWith("image/") || file.type.startsWith("video/"),
    );
  }

  function point(event: DragEvent, insideFrame: boolean): Point {
    if (!insideFrame || !frame) return { x: event.clientX, y: event.clientY };
    const rect = frame.getBoundingClientRect();
    return {
      x: event.clientX + rect.left + frame.clientLeft,
      y: event.clientY + rect.top + frame.clientTop,
    };
  }

  function enter(event: DragEvent): void {
    if (!isFileDrag(event) || busy) return;
    refresh();
    active(true);
    root.setAttribute("data-error", "false");
    message.textContent = READY;
  }

  function over(event: DragEvent, insideFrame: boolean): void {
    if (!isFileDrag(event) || busy) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
    if (root.getAttribute("data-active") !== "true") enter(event);

    geometry = read();
    const at = point(event, insideFrame);
    paint(geometry ? insertion(geometry, at.x, at.y) : undefined);
  }

  async function drop(event: DragEvent, insideFrame: boolean): Promise<void> {
    if (!isFileDrag(event) || busy) return;
    event.preventDefault();

    geometry = read();
    const at = point(event, insideFrame);
    const destination = geometry ? insertion(geometry, at.x, at.y) : undefined;
    const files = mediaFiles(event);
    if (!destination || files.length === 0) {
      status.textContent =
        files.length === 0 ? "Only image and video files can be added." : OUTSIDE;
      active(false);
      return;
    }

    paint(destination);
    const droppedOn = slide;
    busy = true;
    root.setAttribute("data-busy", "true");
    root.setAttribute("data-error", "false");
    message.textContent = `Adding ${files.length === 1 ? files[0]!.name : `${files.length} files`}…`;

    let to = destination.to;
    try {
      for (const file of files) {
        const uploaded = await handlers.upload(file);
        await handlers.run({
          op: "insertMedia",
          slide: droppedOn,
          at: to,
          region: destination.region,
          kind: uploaded.kind,
          src: uploaded.src,
          alt: uploaded.alt,
        });
        to += 1;
      }

      status.textContent = `${files.length} media ${files.length === 1 ? "file" : "files"} added.`;
      active(false);
    } catch (error) {
      const detail = error instanceof Error ? error.message : "The media files could not be added.";
      status.textContent = detail;
      message.textContent = `${detail} Click or press Escape to dismiss.`;
      root.setAttribute("data-error", "true");
      active(true);
    } finally {
      busy = false;
      root.setAttribute("data-busy", "false");
    }
  }

  function leave(event: DragEvent): void {
    if (busy || root.getAttribute("data-error") === "true") return;
    if (event.relatedTarget === null) active(false);
  }

  function dismiss(): void {
    if (root.getAttribute("data-error") !== "true") return;
    root.setAttribute("data-error", "false");
    active(false);
  }

  function keydown(event: KeyboardEvent): void {
    if (event.key !== "Escape" || root.getAttribute("data-active") !== "true") return;
    event.preventDefault();
    root.setAttribute("data-error", "false");
    active(false);
  }

  const documentEnter = (event: DragEvent) => enter(event);
  const documentOver = (event: DragEvent) => over(event, false);
  const documentDrop = (event: DragEvent) => void drop(event, false);
  const frameEnter = (event: DragEvent) => enter(event);
  const frameOver = (event: DragEvent) => over(event, true);
  const frameDrop = (event: DragEvent) => void drop(event, true);

  function unbindPage(): void {
    page?.removeEventListener("dragenter", frameEnter);
    page?.removeEventListener("dragover", frameOver);
    page?.removeEventListener("drop", frameDrop);
    page?.removeEventListener("dragleave", leave);
    page = undefined;
  }

  function bindPage(): void {
    unbindPage();
    page = frame?.contentDocument ?? undefined;
    page?.addEventListener("dragenter", frameEnter);
    page?.addEventListener("dragover", frameOver);
    page?.addEventListener("drop", frameDrop);
    page?.addEventListener("dragleave", leave);
    refresh();
  }

  function bindFrame(): void {
    const next = root.ownerDocument.querySelector<HTMLIFrameElement>(".slidx-canvas-frame");
    if (next === frame) return;
    frame?.removeEventListener("load", bindPage);
    unbindPage();
    frame = next ?? undefined;
    frame?.addEventListener("load", bindPage);
    bindPage();
  }

  const document = root.ownerDocument;
  document.addEventListener("dragenter", documentEnter);
  document.addEventListener("dragover", documentOver);
  document.addEventListener("drop", documentDrop);
  document.addEventListener("dragleave", leave);
  document.addEventListener("keydown", keydown);
  document.defaultView?.addEventListener("resize", refresh);
  message.addEventListener("click", dismiss);

  return {
    root,
    render(state) {
      slide = state.selection.slide;
      bindFrame();
      refresh();
    },
    destroy() {
      if (destroyed) return;
      destroyed = true;
      document.removeEventListener("dragenter", documentEnter);
      document.removeEventListener("dragover", documentOver);
      document.removeEventListener("drop", documentDrop);
      document.removeEventListener("dragleave", leave);
      document.removeEventListener("keydown", keydown);
      document.defaultView?.removeEventListener("resize", refresh);
      frame?.removeEventListener("load", bindPage);
      unbindPage();
    },
  };
}
