import { afterEach, describe, expect, it } from "vite-plus/test";

import type { SlideKind } from "../src/operations";
import { createSlideMenu, type SlideMenu } from "../src/slide-menu";

let active: SlideMenu | undefined;

afterEach(() => {
  active?.destroy();
  active = undefined;
  document.body.replaceChildren();
});

function menu(pick: (kind: SlideKind) => void = () => {}): SlideMenu {
  active = createSlideMenu(pick);
  document.body.append(active.root);
  return active;
}

describe("the new-slide menu", () => {
  it("offers narrative starting points rather than source templates", () => {
    const picked: SlideKind[] = [];
    const slides = menu((kind) => picked.push(kind));
    const toggle = slides.root.querySelector<HTMLButtonElement>(".slidx-slide-add-toggle")!;

    slides.open();
    const choices = [...slides.root.querySelectorAll<HTMLButtonElement>("[data-slide-kind]")];

    expect(toggle.getAttribute("aria-haspopup")).toBe("menu");
    expect(slides.root.querySelector(`#${toggle.getAttribute("aria-controls")}`)).not.toBeNull();
    expect(choices.map((choice) => choice.dataset.slideKind)).toEqual([
      "title-body",
      "statement",
      "comparison",
      "points",
    ]);
    expect(choices.map((choice) => choice.querySelectorAll("i").length)).toEqual([2, 1, 2, 4]);

    choices[2]!.click();
    expect(picked).toEqual(["comparison"]);
    expect(slides.root.querySelector<HTMLElement>("[role=menu]")!.hidden).toBe(true);
  });

  it("walks the compositions with arrows and restores focus on escape", () => {
    const slides = menu();
    const toggle = slides.root.querySelector<HTMLButtonElement>(".slidx-slide-add-toggle")!;
    const choices = [...slides.root.querySelectorAll<HTMLButtonElement>("[data-slide-kind]")];

    slides.open();
    expect(document.activeElement).toBe(choices[0]);

    choices[0]!.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true }));
    expect(document.activeElement).toBe(choices[3]);

    choices[3]!.dispatchEvent(new KeyboardEvent("keydown", { key: "Home", bubbles: true }));
    expect(document.activeElement).toBe(choices[0]);

    choices[0]!.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(document.activeElement).toBe(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
  });
});
