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
    const root = document.createElement("html");
    const slide = document.createElement("article");
    slide.className = "slidx-slide";
    const block = document.createElement("p");
    block.setAttribute("data-slidx-element-color", "");
    block.setAttribute("data-slidx-freeform-frame", "");
    block.style.setProperty("--slidx-element-color", "var(--slidx-block-id-result-color)");
    slide.style.setProperty("--slidx-block-id-result-color", "var(--slidx-color-accent)");
    slide.append(block);
    const colors: Record<string, string> = {
      "--slidx-color-text": "#161b22",
      "--slidx-color-heading": "#0d1218",
      "--slidx-color-muted": "#5f656e",
      "--slidx-color-accent": "#a5c9ff",
    };
    const frame = {
      contentDocument: { querySelector: () => block, documentElement: root },
      contentWindow: {
        getComputedStyle: (element: Element) =>
          element === block
            ? { color: "rgb(165, 201, 255)" }
            : { getPropertyValue: (name: string) => colors[name] ?? "" },
      },
    } as unknown as HTMLIFrameElement;

    expect(visualOf(frame, 2)).toEqual({
      color: "rgb(165, 201, 255)",
      managedColor: true,
      managedFrame: true,
      managedValue: "var(--slidx-color-accent)",
      themeColor: "accent",
      palette: [
        { name: "text", label: "Text", color: "#161b22" },
        { name: "heading", label: "Heading", color: "#0d1218" },
        { name: "muted", label: "Muted", color: "#5f656e" },
        { name: "accent", label: "Accent", color: "#a5c9ff" },
      ],
    });
  });

  it("keeps a fixed color distinct even when it currently matches the theme", () => {
    const root = document.createElement("html");
    const slide = document.createElement("article");
    slide.className = "slidx-slide";
    const block = document.createElement("p");
    block.setAttribute("data-slidx-element-color", "");
    block.style.setProperty("--slidx-element-color", "var(--slidx-block-id-result-color)");
    slide.style.setProperty("--slidx-block-id-result-color", "#a5c9ff");
    slide.append(block);
    const frame = {
      contentDocument: { querySelector: () => block, documentElement: root },
      contentWindow: {
        getComputedStyle: (element: Element) =>
          element === block
            ? { color: "rgb(165, 201, 255)" }
            : { getPropertyValue: () => "#a5c9ff" },
      },
    } as unknown as HTMLIFrameElement;

    expect(visualOf(frame, 2)?.managedValue).toBe("#a5c9ff");
    expect(visualOf(frame, 2)?.themeColor).toBeUndefined();
  });
});
