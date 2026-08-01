/** A reversible workspace that keeps the rendered slide useful at every width. */

import { element } from "./dom";
import { applyWorkspaceFocusStyles } from "./workspace-focus-styles";

export interface WorkspaceFocusOptions {
  /** Re-measures overlays after the panel grid changes shape. */
  changed?(): void;
  /** Remembers which compact side panel the author last kept beside the canvas. */
  storage?: Storage | undefined;
}

export type WorkspacePanel = "outline" | "canvas" | "inspector";

export interface WorkspaceFocus {
  /** The compact group mounted in the appbar. */
  root: HTMLElement;
  /** The existing full-canvas command, kept public for shortcuts and tests. */
  trigger: HTMLButtonElement;
  active(): boolean;
  panel(): WorkspacePanel;
  connect(): void;
  togglePanel(panel: Exclude<WorkspacePanel, "canvas">): void;
  toggle(): void;
  exit(): void;
}

const PANEL_STORAGE_KEY = "slidx.editor.workspace-panel";

/** Creates the appbar controls and the local, non-document workspace state they own. */
export function createWorkspaceFocus(options: WorkspaceFocusOptions = {}): WorkspaceFocus {
  let selected = storedPanel(options.storage) ?? "outline";
  const label = element("span", { class: "slidx-workspace-focus-label" }, ["Focus"]);
  const trigger = element(
    "button",
    {
      type: "button",
      class: "slidx-workspace-focus",
      title: "Focus canvas (F)",
      "aria-label": "Focus canvas",
      "aria-pressed": "false",
      "aria-keyshortcuts": "F",
      "data-active": "false",
    },
    [element("span", { class: "slidx-workspace-focus-icon", "aria-hidden": "true" }), label],
  ) as HTMLButtonElement;
  const outline = panelButton("outline", "Slides");
  const inspector = panelButton("inspector", "Properties");
  const root = element(
    "div",
    { class: "slidx-workspace-controls", role: "group", "aria-label": "Workspace panels" },
    [outline, trigger, inspector],
  );
  applyWorkspaceFocusStyles(trigger.ownerDocument);

  const editor = () => root.closest<HTMLElement>(".slidx-editor");

  function active(): boolean {
    return editor()?.getAttribute("data-canvas-focus") === "true";
  }

  function panel(): WorkspacePanel {
    return selected;
  }

  function sync(): void {
    const focused = active();
    const outlineVisible = !focused && selected === "outline";
    const inspectorVisible = !focused && selected === "inspector";

    panelState(outline, outlineVisible, "slides");
    panelState(inspector, inspectorVisible, "inspector");
  }

  function connect(): void {
    const frame = editor();
    if (!frame) return;

    frame.setAttribute("data-workspace-panel", selected);
    sync();
  }

  function set(next: boolean, restore = true): void {
    const frame = editor();
    if (!frame || active() === next) return;

    frame.setAttribute("data-canvas-focus", String(next));
    trigger.setAttribute("aria-pressed", String(next));
    trigger.setAttribute("data-active", String(next));
    trigger.setAttribute("aria-label", next ? "Restore workspace" : "Focus canvas");
    trigger.title = next ? "Restore workspace (F or Escape)" : "Focus canvas (F)";
    label.textContent = next ? "Restore" : "Focus";
    sync();
    options.changed?.();

    if (next) frame.querySelector<HTMLIFrameElement>(".slidx-canvas-frame")?.focus();
    else if (restore) trigger.focus();
  }

  function togglePanel(next: Exclude<WorkspacePanel, "canvas">): void {
    const frame = editor();
    if (!frame) return;

    const wasFocused = active();
    if (wasFocused) set(false, false);

    selected = !wasFocused && selected === next ? "canvas" : next;
    frame.setAttribute("data-workspace-panel", selected);
    storePanel(options.storage, selected);
    sync();
    options.changed?.();
  }

  const focus: WorkspaceFocus = {
    root,
    trigger,
    active,
    panel,
    connect,
    togglePanel,
    toggle: () => set(!active()),
    exit: () => set(false),
  };
  trigger.addEventListener("click", () => focus.toggle());
  outline.addEventListener("click", () => focus.togglePanel("outline"));
  inspector.addEventListener("click", () => focus.togglePanel("inspector"));

  return focus;
}

function panelButton(panel: "outline" | "inspector", label: string): HTMLButtonElement {
  return element(
    "button",
    {
      type: "button",
      class: "slidx-workspace-panel",
      "data-panel": panel,
      "aria-label": `Show ${label.toLowerCase()} panel`,
      "aria-pressed": "false",
      title: `Show ${label.toLowerCase()} panel`,
    },
    [
      element("span", {
        class: "slidx-workspace-panel-icon",
        "data-panel": panel,
        "aria-hidden": "true",
      }),
      element("span", { class: "slidx-workspace-panel-label" }, [label]),
    ],
  ) as HTMLButtonElement;
}

function panelState(button: HTMLButtonElement, visible: boolean, name: string): void {
  button.setAttribute("aria-pressed", String(visible));
  button.setAttribute("data-active", String(visible));
  button.setAttribute("aria-label", `${visible ? "Hide" : "Show"} ${name} panel`);
  button.title = `${visible ? "Hide" : "Show"} ${name} panel`;
}

function storedPanel(storage: Storage | undefined): WorkspacePanel | undefined {
  try {
    const panel = storage?.getItem(PANEL_STORAGE_KEY);
    return panel === "outline" || panel === "canvas" || panel === "inspector" ? panel : undefined;
  } catch {
    return undefined;
  }
}

function storePanel(storage: Storage | undefined, panel: WorkspacePanel): void {
  try {
    storage?.setItem(PANEL_STORAGE_KEY, panel);
  } catch {
    // A private or quota-limited browser still gets a complete in-memory workspace.
  }
}
