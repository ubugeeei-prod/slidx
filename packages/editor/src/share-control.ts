/**
 * The local author's handoff sheet.
 *
 * The links contain capabilities, so this surface never discovers or composes
 * them. It asks the dev server, whose local-only route is the security
 * boundary, and only holds the answer long enough to copy one. Invited
 * browsers receive `null` and never see the control at all.
 */

import type { SharingInfo } from "./client";
import { element } from "./dom";
import type { Surface } from "./outline";
import { applyShareControlStyles } from "./share-control-styles";

export interface ShareControlOptions {
  load(): Promise<SharingInfo | null>;
  copy?: (value: string) => Promise<void>;
}

export interface ShareControl extends Surface {
  /** Settles when the local-only route has decided whether this control exists. */
  ready: Promise<void>;
}

/** A compact command-bar trigger and its flat, capability-aware handoff sheet. */
export function createShareControl(options: ShareControlOptions): ShareControl {
  const titleId = `slidx-share-title-${++nextShare}`;
  const eyebrow = element("span", { class: "slidx-share-eyebrow" });
  const title = element("h2", { id: titleId, class: "slidx-share-title" });
  const close = element(
    "button",
    { type: "button", class: "slidx-share-close", title: "Close", "aria-label": "Close share" },
    ["×"],
  ) as HTMLButtonElement;
  const body = element("div", { class: "slidx-share-body" });
  const popover = element(
    "section",
    {
      class: "slidx-share-popover",
      role: "dialog",
      "aria-modal": "false",
      "aria-labelledby": titleId,
      hidden: true,
    },
    [
      element("header", { class: "slidx-share-head" }, [
        element("div", { class: "slidx-share-heading" }, [eyebrow, title]),
        close,
      ]),
      body,
    ],
  );
  const toggle = element(
    "button",
    {
      type: "button",
      class: "slidx-share-toggle",
      title: "Share this deck",
      "aria-label": "Share",
      "aria-haspopup": "dialog",
      "aria-expanded": "false",
      disabled: true,
    },
    [
      element("span", { class: "slidx-share-toggle-mark", "aria-hidden": "true" }, ["↗"]),
      element("span", { class: "slidx-share-toggle-label" }, ["Share"]),
    ],
  ) as HTMLButtonElement;
  const root = element("div", { class: "slidx-share", hidden: true, "data-open": "false" }, [
    toggle,
    popover,
  ]);
  const document = root.ownerDocument;
  const copy = options.copy ?? ((value: string) => copyText(document, value));
  let firstAction: HTMLButtonElement = close;

  function open(): void {
    if (root.hidden || toggle.disabled) return;
    popover.hidden = false;
    root.dataset.open = "true";
    toggle.setAttribute("aria-expanded", "true");
    firstAction.focus();
  }

  function shut(restore = false): void {
    if (popover.hidden) return;
    popover.hidden = true;
    root.dataset.open = "false";
    toggle.setAttribute("aria-expanded", "false");
    if (restore) toggle.focus();
  }

  toggle.addEventListener("click", () => (popover.hidden ? open() : shut(true)));
  close.addEventListener("click", () => shut(true));

  const dismiss = (event: PointerEvent) => {
    if (event.target instanceof Node && !root.contains(event.target)) shut();
  };
  const escape = (event: KeyboardEvent) => {
    if (event.key !== "Escape" || popover.hidden) return;
    event.preventDefault();
    shut(true);
  };
  document.addEventListener("pointerdown", dismiss);
  document.addEventListener("keydown", escape);
  applyShareControlStyles(document);

  const ready = options
    .load()
    .then((sharing) => {
      if (sharing === null) return;

      root.hidden = false;
      toggle.disabled = false;
      show(sharing);
    })
    .catch(() => {
      root.hidden = false;
      toggle.disabled = false;
      showUnavailable();
    });

  function show(sharing: SharingInfo): void {
    if (!sharing.enabled) {
      eyebrow.textContent = "Local session";
      title.textContent = "Sharing is off";
      body.replaceChildren(
        element("p", { class: "slidx-share-intro" }, [
          "Restart the dev server with sharing enabled to hand this working deck to someone nearby.",
        ]),
        element("code", { class: "slidx-share-command" }, ["slidx dev --crdt"]),
        element("p", { class: "slidx-share-foot" }, [
          "Add --allow-edit only when the other person should be able to rewrite slide files.",
        ]),
      );
      firstAction = close;
      return;
    }

    eyebrow.textContent = "Live handoff";
    title.textContent = "Share this working deck";
    const links = element("div", { class: "slidx-share-links" });
    const status = element("p", {
      class: "slidx-share-status",
      role: "status",
      "aria-live": "polite",
    });
    const actions: HTMLButtonElement[] = [];

    if (sharing.read) {
      const row = linkRow(
        "read",
        "View only",
        "Can follow the deck, never rewrite files",
        sharing.read,
        status,
      );
      links.append(row.root);
      actions.push(row.button);
    }
    if (sharing.edit) {
      const row = linkRow(
        "edit",
        "Can edit",
        "Changes are written to this project",
        sharing.edit,
        status,
      );
      links.append(row.root);
      actions.push(row.button);
    }

    const intro = element("p", { class: "slidx-share-intro" }, [
      actions.length > 0
        ? "Choose the smallest permission that lets the other person help."
        : "This session is shared. Its links remain available in the terminal that started it.",
    ]);
    body.replaceChildren(
      intro,
      links,
      element("p", { class: "slidx-share-foot" }, [
        "Anyone on this network who has a link can use it. Every link stops with this dev server.",
      ]),
      status,
    );
    firstAction = actions[0] ?? close;
  }

  function showUnavailable(): void {
    eyebrow.textContent = "Local session";
    title.textContent = "Share status unavailable";
    body.replaceChildren(
      element("p", { class: "slidx-share-intro" }, [
        "The deck is still open, but its handoff links could not be read. Check the terminal running the dev server.",
      ]),
    );
    firstAction = close;
  }

  function linkRow(
    kind: "read" | "edit",
    label: string,
    detail: string,
    value: string,
    status: HTMLElement,
  ): { root: HTMLElement; button: HTMLButtonElement } {
    const button = element(
      "button",
      { type: "button", class: "slidx-share-copy", "data-copied": "false" },
      ["Copy link"],
    ) as HTMLButtonElement;
    button.addEventListener("click", () => {
      void copy(value)
        .then(() => {
          for (const other of body.querySelectorAll<HTMLButtonElement>(".slidx-share-copy")) {
            other.textContent = other === button ? "Copied" : "Copy link";
            other.dataset.copied = String(other === button);
          }
          status.textContent = `${label} link copied.`;
        })
        .catch(() => {
          status.textContent = "Copy failed. The link is still available in the terminal.";
        });
    });

    let host = "this network";
    try {
      host = new URL(value).host;
    } catch {
      // The server owns validation. A malformed injected test value still gets
      // a useful label rather than breaking the whole command bar.
    }

    return {
      root: element("div", { class: "slidx-share-link", "data-kind": kind }, [
        element("span", { class: "slidx-share-link-copy" }, [
          element("span", { class: "slidx-share-link-label" }, [label]),
          element("span", { class: "slidx-share-link-detail" }, [`${detail} · ${host}`]),
        ]),
        button,
      ]),
      button,
    };
  }

  return {
    root,
    ready,
    render() {},
    destroy() {
      document.removeEventListener("pointerdown", dismiss);
      document.removeEventListener("keydown", escape);
    },
  };
}

let nextShare = 0;

async function copyText(document: Document, value: string): Promise<void> {
  const clipboard = document.defaultView?.navigator.clipboard;
  if (clipboard) {
    await clipboard.writeText(value);
    return;
  }

  const field = document.createElement("textarea");
  field.value = value;
  field.setAttribute("readonly", "");
  field.style.position = "fixed";
  field.style.opacity = "0";
  document.body.append(field);
  field.select();

  const copied = typeof document.execCommand === "function" && document.execCommand("copy");
  field.remove();
  if (!copied) throw new Error("The browser did not make clipboard access available.");
}
