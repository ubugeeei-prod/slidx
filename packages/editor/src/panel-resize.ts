/**
 * The width of the two side panels.
 *
 * This is a viewing preference rather than deck content, so it lives in this
 * browser and never becomes an edit operation. The separators sit over the
 * existing three-column grid: the timeline owns that grid's area declaration,
 * and adding two structural columns here would make the two surfaces disagree.
 */

import { element } from "./dom";
import type { Surface } from "./outline";

type Panel = "outline" | "inspector";

interface PanelSpec {
  initial: number;
  min: number;
  max: number;
  property: string;
  storage: string;
}

const CANVAS_MIN = 360;
const STEP = 16;

const PANELS: Record<Panel, PanelSpec> = {
  outline: {
    initial: 232,
    min: 176,
    max: 480,
    property: "--slidx-e-outline-width",
    storage: "slidx.editor.outline-width",
  },
  inspector: {
    initial: 296,
    min: 240,
    max: 520,
    property: "--slidx-e-inspector-width",
    storage: "slidx.editor.inspector-width",
  },
};

export interface PanelResizeOptions {
  storage?: Pick<Storage, "getItem" | "setItem"> | undefined;
}

/** Adds accessible pointer and keyboard separators for the side panels. */
export function createPanelResize(options: PanelResizeOptions = {}): Surface {
  const separators = (Object.keys(PANELS) as Panel[]).map((panel) => separator(panel));
  const root = element(
    "div",
    { class: "slidx-panel-resizers", "aria-label": "Panel sizes" },
    separators,
  );

  let editor: HTMLElement | undefined;
  let initialized = false;
  let dragging: Panel | undefined;

  function widthOf(panel: Panel): number {
    const spec = PANELS[panel];
    const written = editor?.style.getPropertyValue(spec.property);
    const parsed = Number.parseFloat(written ?? "");

    return Number.isFinite(parsed) ? parsed : spec.initial;
  }

  function available(panel: Panel): number {
    const other = panel === "outline" ? "inspector" : "outline";
    const viewport =
      editor?.getBoundingClientRect().width || root.ownerDocument.defaultView?.innerWidth;

    return Math.max(PANELS[panel].min, (viewport || 1280) - widthOf(other) - CANVAS_MIN);
  }

  function clamp(panel: Panel, wanted: number): number {
    const spec = PANELS[panel];
    return Math.round(Math.max(spec.min, Math.min(wanted, spec.max, available(panel))));
  }

  function set(panel: Panel, wanted: number, remember = true): void {
    if (!editor || !Number.isFinite(wanted)) return;

    const spec = PANELS[panel];
    const width = clamp(panel, wanted);
    editor.style.setProperty(spec.property, `${width}px`);

    const control = separators.find((candidate) => candidate.dataset.panel === panel)!;
    control.setAttribute("aria-valuenow", String(width));
    control.setAttribute("aria-valuetext", `${width} pixels`);
    if (remember) options.storage?.setItem(spec.storage, String(width));

    // Changing a grid track does not produce a window resize event. Every
    // canvas overlay already re-measures on that event, so explicitly sending
    // it keeps grips, guides, collaborator marks and drop targets on the slide.
    root.ownerDocument.defaultView?.dispatchEvent(new Event("resize"));
  }

  function restore(panel: Panel): void {
    const saved = Number(options.storage?.getItem(PANELS[panel].storage));
    set(panel, Number.isFinite(saved) && saved > 0 ? saved : PANELS[panel].initial, false);
  }

  function point(panel: Panel, event: PointerEvent): void {
    if (!editor || dragging !== panel) return;

    const rect = editor.getBoundingClientRect();
    set(panel, panel === "outline" ? event.clientX - rect.left : rect.right - event.clientX);
  }

  for (const control of separators) {
    const panel = control.dataset.panel as Panel;

    control.addEventListener("pointerdown", (event) => {
      event.preventDefault();
      // Preventing native selection also prevents the browser's usual
      // pointer-to-focus step, so restore it explicitly. The next arrow must
      // keep resizing this separator rather than becoming a deck command.
      control.focus();
      dragging = panel;
      control.setPointerCapture?.(event.pointerId);
      control.setAttribute("data-dragging", "true");
      point(panel, event);
    });
    control.addEventListener("pointermove", (event) => point(panel, event));
    control.addEventListener("pointerup", () => {
      dragging = undefined;
      control.setAttribute("data-dragging", "false");
    });
    control.addEventListener("pointercancel", () => {
      dragging = undefined;
      control.setAttribute("data-dragging", "false");
    });
    control.addEventListener("dblclick", () => set(panel, PANELS[panel].initial));
    control.addEventListener("keydown", (event) => {
      const direction = panel === "outline" ? 1 : -1;
      const delta =
        event.key === "ArrowRight"
          ? STEP * direction
          : event.key === "ArrowLeft"
            ? -STEP * direction
            : 0;

      if (event.key === "Home") {
        event.preventDefault();
        set(panel, PANELS[panel].min);
      } else if (event.key === "End") {
        event.preventDefault();
        set(panel, PANELS[panel].max);
      } else if (delta !== 0) {
        event.preventDefault();
        set(panel, widthOf(panel) + delta);
      }
    });
  }

  return {
    root,
    render() {
      if (initialized) return;
      editor = root.closest<HTMLElement>(".slidx-editor") ?? undefined;
      if (!editor) return;

      initialized = true;
      restore("outline");
      restore("inspector");
    },
  };
}

function separator(panel: Panel): HTMLElement {
  const spec = PANELS[panel];

  return element("div", {
    class: "slidx-panel-resizer",
    "data-panel": panel,
    "data-dragging": false,
    role: "separator",
    tabindex: 0,
    "aria-label": panel === "outline" ? "Resize slide panel" : "Resize inspector panel",
    "aria-orientation": "vertical",
    "aria-valuemin": spec.min,
    "aria-valuemax": spec.max,
    "aria-valuenow": spec.initial,
    "aria-valuetext": `${spec.initial} pixels`,
  });
}
