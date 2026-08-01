import { afterEach, describe, expect, it } from "vite-plus/test";

import { createContentMenu, type ContentMenu } from "../src/content-menu";
import type { BlockKind } from "../src/operations";

let active: ContentMenu | undefined;

afterEach(() => {
  active?.destroy();
  active = undefined;
  document.body.replaceChildren();
});

function menu(pick: (kind: BlockKind) => void = () => {}): ContentMenu {
  active = createContentMenu(pick);
  document.body.append(active.root);
  return active;
}

describe("the add-content menu", () => {
  it("offers common blocks as semantic choices", () => {
    const picked: BlockKind[] = [];
    const content = menu((kind) => picked.push(kind));

    content.open();
    const toggle = content.root.querySelector<HTMLButtonElement>(".slidx-content-toggle")!;
    const choices = [...content.root.querySelectorAll<HTMLButtonElement>("[data-kind]")];

    expect(toggle.getAttribute("aria-haspopup")).toBe("menu");
    expect(content.root.querySelector(`#${toggle.getAttribute("aria-controls")}`)).not.toBeNull();
    expect(choices.map((choice) => choice.dataset.kind)).toEqual([
      "heading",
      "text",
      "list",
      "quote",
    ]);
    choices[3]!.click();
    expect(picked).toEqual(["quote"]);
    expect(content.root.querySelector<HTMLElement>("[role=menu]")!.hidden).toBe(true);
  });

  it("moves through choices with the keyboard and returns focus on escape", () => {
    const content = menu();
    const toggle = content.root.querySelector<HTMLButtonElement>(".slidx-content-toggle")!;
    const choices = [...content.root.querySelectorAll<HTMLButtonElement>("[data-kind]")];

    content.open();
    expect(document.activeElement).toBe(choices[0]);

    choices[0]!.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    expect(document.activeElement).toBe(choices[1]);

    choices[1]!.dispatchEvent(new KeyboardEvent("keydown", { key: "End", bubbles: true }));
    expect(document.activeElement).toBe(choices[3]);

    choices[3]!.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(document.activeElement).toBe(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
  });

  it("light-dismisses when the author presses outside it", () => {
    const content = menu();
    content.open();

    document.body.dispatchEvent(new Event("pointerdown", { bubbles: true }));

    expect(content.root.querySelector<HTMLElement>("[role=menu]")!.hidden).toBe(true);
  });

  it("cannot open while visual insertion is unavailable", () => {
    const content = menu();
    const toggle = content.root.querySelector<HTMLButtonElement>(".slidx-content-toggle")!;
    content.setEnabled(false);

    content.open();

    expect(toggle.disabled).toBe(true);
    expect(content.root.querySelector<HTMLElement>("[role=menu]")!.hidden).toBe(true);
  });
});
