/** The compact collaboration chip and the roster it reveals. */

import type { Viewer } from "./collab";
import { element, fill } from "./dom";
import { applyPresenceStyles } from "./presence-styles";

let nextPanel = 0;

export interface PresenceView {
  root: HTMLElement;
  draw(viewers: Viewer[], following: string | undefined): void;
  destroy(): void;
}

export function createPresenceView(
  follow: ((seat: string | undefined) => void) | undefined,
): PresenceView {
  const panelId = `slidx-presence-${++nextPanel}`;
  const avatars = element("span", { class: "slidx-presence-avatars", "aria-hidden": "true" });
  const count = element("span", { class: "slidx-presence-count" });
  const total = element("span", { class: "slidx-presence-total" });
  const list = element("ul", { class: "slidx-presence-list" });
  const panel = element(
    "div",
    {
      id: panelId,
      class: "slidx-presence-popover",
      hidden: true,
      "aria-label": "People editing this deck",
    },
    [
      element("div", { class: "slidx-presence-head" }, [
        element("strong", { class: "slidx-presence-label" }, ["Editing together"]),
        total,
      ]),
      list,
    ],
  );
  const toggle = element(
    "button",
    {
      type: "button",
      class: "slidx-presence-toggle",
      "aria-expanded": "false",
      "aria-controls": panelId,
      "aria-haspopup": "true",
    },
    [avatars, count],
  ) as HTMLButtonElement;
  const root = element(
    "aside",
    { class: "slidx-presence", "data-empty": true, "data-open": false },
    [toggle, panel],
  );
  applyPresenceStyles(root.ownerDocument);

  let shown: Viewer[] = [];
  let following: string | undefined;
  let open = false;

  function setOpen(next: boolean): void {
    open = next && shown.length >= 2;
    root.setAttribute("data-open", String(open));
    toggle.setAttribute("aria-expanded", String(open));
    panel.hidden = !open;

    if (open) root.ownerDocument.addEventListener("pointerdown", dismiss);
    else root.ownerDocument.removeEventListener("pointerdown", dismiss);
  }

  function dismiss(event: PointerEvent): void {
    if (event.target instanceof Node && !root.contains(event.target)) setOpen(false);
  }

  function row(viewer: Viewer): HTMLElement {
    const inside = [
      element("span", { class: "slidx-presence-name" }, [viewer.label]),
      element("span", { class: "slidx-presence-where" }, [`slide ${viewer.slide + 1}`]),
      ...(viewer.canEdit ? [] : [element("span", { class: "slidx-presence-role" }, ["reading"])]),
    ];

    if (viewer.local || follow === undefined) {
      return element("span", { class: "slidx-presence-seat", "data-local": viewer.local }, inside);
    }

    const button = element(
      "button",
      {
        class: "slidx-presence-seat",
        type: "button",
        "data-local": false,
        "aria-pressed": String(viewer.id === following),
        title:
          viewer.id === following ? `Stop following ${viewer.label}` : `Follow ${viewer.label}`,
      },
      inside,
    );
    button.addEventListener("click", () => follow(viewer.id === following ? undefined : viewer.id));
    return button;
  }

  function draw(viewers: Viewer[], followed: string | undefined): void {
    shown = viewers;
    following = followed;
    root.setAttribute("data-empty", String(shown.length < 2));
    if (shown.length < 2) setOpen(false);

    toggle.setAttribute("aria-label", `${shown.length} people editing this deck`);
    fill(
      avatars,
      shown
        .slice(0, 3)
        .map((viewer) =>
          element("span", { class: "slidx-presence-avatar", title: viewer.label }, [
            initial(viewer.label),
          ]),
        ),
    );
    fill(count, [
      String(shown.length),
      element("span", { class: "slidx-presence-count-suffix" }, [" here"]),
    ]);
    total.textContent = `${shown.length} people`;
    fill(
      list,
      shown.map((viewer) => element("li", { class: "slidx-presence-who" }, [row(viewer)])),
    );
  }

  toggle.addEventListener("click", () => setOpen(!open));
  root.addEventListener("keydown", (event) => {
    if (event.key !== "Escape" || !open) return;
    setOpen(false);
    toggle.focus();
  });

  return {
    root,
    draw,
    destroy() {
      root.ownerDocument.removeEventListener("pointerdown", dismiss);
    },
  };
}

/** One visible character without breaking a multi-byte label. */
function initial(label: string): string {
  return Array.from(label.trim())[0]?.toLocaleUpperCase() ?? "•";
}
