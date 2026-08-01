/** The local author's safe, reusable handoff sheet. */

import { afterEach, describe, expect, it } from "vite-plus/test";

import type { SharingInfo } from "../src/client";
import { createShareControl } from "../src/share-control";

const surfaces: ReturnType<typeof createShareControl>[] = [];

afterEach(() => {
  for (const surface of surfaces.splice(0)) surface.destroy?.();
  document.body.replaceChildren();
});

async function open(
  sharing: SharingInfo | null,
  copied: string[] = [],
): Promise<ReturnType<typeof createShareControl>> {
  const surface = createShareControl({
    load: async () => sharing,
    copy: async (value) => {
      copied.push(value);
    },
  });
  surfaces.push(surface);
  document.body.append(surface.root);
  await surface.ready;
  return surface;
}

describe("the share control", () => {
  it("does not exist in an invited browser", async () => {
    const surface = await open(null);

    expect(surface.root.hidden).toBe(true);
    expect(surface.root.querySelector<HTMLButtonElement>('[aria-label="Share"]')!.disabled).toBe(
      true,
    );
  });

  it("explains how to start a handoff when sharing is off", async () => {
    const surface = await open({ enabled: false });
    const toggle = surface.root.querySelector<HTMLButtonElement>('[aria-label="Share"]')!;

    toggle.click();

    expect(surface.root.hidden).toBe(false);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(surface.root.querySelector('[role="dialog"]')!.textContent).toContain("Sharing is off");
    expect(surface.root.querySelector(".slidx-share-command")!.textContent).toBe(
      "slidx dev --crdt",
    );
  });

  it("copies the exact capability chosen and distinguishes editing before the click", async () => {
    const copied: string[] = [];
    const read = "http://192.168.1.42:5173/__slidx/#s=reader";
    const edit = "http://192.168.1.42:5173/__slidx/#s=editor";
    const surface = await open({ enabled: true, read, edit }, copied);
    const toggle = surface.root.querySelector<HTMLButtonElement>('[aria-label="Share"]')!;

    toggle.click();
    const rows = [...surface.root.querySelectorAll<HTMLElement>(".slidx-share-link")];
    const buttons = [...surface.root.querySelectorAll<HTMLButtonElement>(".slidx-share-copy")];

    expect(rows.map((row) => row.dataset.kind)).toEqual(["read", "edit"]);
    expect(rows[0]!.textContent).toContain("never rewrite files");
    expect(rows[1]!.textContent).toContain("written to this project");
    expect(document.activeElement).toBe(buttons[0]);

    buttons[1]!.click();
    await Promise.resolve();

    expect(copied).toEqual([edit]);
    expect(buttons[1]!.textContent).toBe("Copied");
    expect(surface.root.querySelector('[role="status"]')!.textContent).toContain("Can edit");
  });

  it("closes on Escape and restores focus to the command bar", async () => {
    const surface = await open({
      enabled: true,
      read: "http://192.168.1.42:5173/__slidx/#s=reader",
    });
    const toggle = surface.root.querySelector<HTMLButtonElement>('[aria-label="Share"]')!;
    const dialog = surface.root.querySelector<HTMLElement>('[role="dialog"]')!;

    toggle.click();
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));

    expect(dialog.hidden).toBe(true);
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(document.activeElement).toBe(toggle);
  });

  it("keeps the editor usable when the optional status route fails", async () => {
    const surface = createShareControl({
      load: async () => {
        throw new Error("gone");
      },
    });
    surfaces.push(surface);
    document.body.append(surface.root);
    await surface.ready;

    surface.root.querySelector<HTMLButtonElement>('[aria-label="Share"]')!.click();

    expect(surface.root.textContent).toContain("Share status unavailable");
    expect(surface.root.textContent).toContain("deck is still open");
  });
});
