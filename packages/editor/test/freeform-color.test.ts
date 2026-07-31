import { describe, expect, it } from "vite-plus/test";

import { hexColor, visualOf } from "../src/freeform-color";

describe("freeform block colour", () => {
  it("normalises short and mixed-case hex for the native picker", () => {
    expect(hexColor(" #A5c ")).toBe("#aa55cc");
    expect(hexColor("#D946Ef")).toBe("#d946ef");
  });

  it("rounds and clamps computed colour channels", () => {
    expect(hexColor("rgba(165.4, 201.6, 300, 0.5)")).toBe("#a5caff");
    expect(hexColor("rgb(0, 7, 15)")).toBe("#00070f");
  });

  it("falls back to a picker-safe value when a colour cannot be represented", () => {
    expect(hexColor("color(display-p3 1 0 0)")).toBe("#000000");
  });

  it("reads the rendered colour and whether the declaration is managed", () => {
    const block = document.createElement("p");
    block.setAttribute("data-slidx-element-color", "");
    const frame = {
      contentDocument: { querySelector: () => block },
      contentWindow: { getComputedStyle: () => ({ color: "rgb(165, 201, 255)" }) },
    } as unknown as HTMLIFrameElement;

    expect(visualOf(frame, 2)).toEqual({
      color: "rgb(165, 201, 255)",
      managedColor: true,
    });
  });
});
