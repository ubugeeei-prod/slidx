/** Visual and semantic controls for one selected block. */

import { sliceBytes } from "./bytes";
import type { BlockSpans } from "./client";
import { element, field } from "./dom";
import { hexColor, type BlockVisual, type ThemeColor } from "./freeform-color";
import {
  alignedFrame,
  FRAME_ANCHORS,
  frameAnchorOf,
  insetOf,
  type FrameAnchor,
} from "./freeform-geometry";
import type { BlockBox, SlideGeometry } from "./geometry";
import type { BlockAttributes, EditOp } from "./operations";
import type { EditorState } from "./session";
import { WIDTHS } from "./widths";

export interface BlockInspectorHandlers {
  run(op: EditOp): void;
  selectBlock?(block: number | undefined): void;
}

export interface BlockInspectorOptions {
  bodyOf(slide: number): string;
  blocksOf?(slide: number): readonly BlockSpans[];
  geometry?(): SlideGeometry | undefined;
  visualOf?(block: number): BlockVisual | undefined;
}

export function blockPanel(
  state: EditorState,
  handlers: BlockInspectorHandlers,
  options: BlockInspectorOptions,
): HTMLElement {
  const index = state.selection.block;
  if (index === undefined) return group("Block", []);

  const span = options.blocksOf?.(state.selection.slide)[index];
  const source = span
    ? sliceBytes(options.bodyOf(state.selection.slide), span.span.start, span.span.end)
    : "";
  // Attached comments carry notes and other source-only metadata. They belong
  // to the block for moves and deletion, but not to the visual summary an
  // author uses to recognise it.
  const summary = source
    .replace(/<!--[\s\S]*?-->/g, "")
    .replace(/\s+/g, " ")
    .trim();
  const geometry = options.geometry?.();
  const box = geometry?.blocks.find((candidate) => candidate.index === index);
  const visual = options.visualOf?.(index);

  return group("Block", [
    element("div", { class: "slidx-block-context" }, [
      element("span", { class: "slidx-block-number" }, [`${index + 1}`]),
      element("p", { class: "slidx-block-source" }, [
        summary.length > 0 ? truncate(summary, 112) : "Selected block",
      ]),
    ]),
    section("Position", [
      regionControl(state, index, box, geometry, handlers),
      frameControl(state, index, box, geometry, visual, handlers),
    ]),
    section("Size", [widthControl(state, index, box, handlers)]),
    section("Appearance", [colorControl(state, index, visual, handlers)]),
    section("Identity", [attributeControl(state, index, span?.attributes, handlers)]),
    element("div", { class: "slidx-block-actions" }, [
      action("Duplicate", "slidx-block-duplicate", () =>
        handlers.run({ op: "duplicateBlock", slide: state.selection.slide, block: index }),
      ),
      action("Delete", "slidx-block-delete", () => {
        handlers.run({ op: "removeBlock", slide: state.selection.slide, block: index });
        handlers.selectBlock?.(undefined);
      }),
    ]),
  ]);
}

const FRAME_ANCHOR_LABELS: Record<FrameAnchor, string> = {
  "top-left": "Top left",
  "top-center": "Top center",
  "top-right": "Top right",
  "middle-left": "Middle left",
  "middle-center": "Center",
  "middle-right": "Middle right",
  "bottom-left": "Bottom left",
  "bottom-center": "Bottom center",
  "bottom-right": "Bottom right",
};

/** Precise placement and, just as importantly, a one-click return to layout flow. */
function frameControl(
  state: EditorState,
  block: number,
  box: BlockBox | undefined,
  geometry: SlideGeometry | undefined,
  visual: BlockVisual | undefined,
  handlers: BlockInspectorHandlers,
): HTMLElement {
  const managed = visual?.managedFrame === true;
  const current = managed && box && geometry ? frameAnchorOf(box.rect, geometry.safe) : undefined;
  const reset = action("Use layout position", "slidx-frame-reset", () =>
    handlers.run({
      op: "setBlockStyle",
      slide: state.selection.slide,
      block,
      property: "inset",
    }),
  );
  reset.disabled = !managed;

  const choices = FRAME_ANCHORS.map((anchor) => {
    const button = element(
      "button",
      {
        type: "button",
        class: "slidx-frame-anchor",
        "data-frame-anchor": anchor,
        "aria-label": `Pin block ${FRAME_ANCHOR_LABELS[anchor].toLowerCase()} in safe area`,
        "aria-pressed": String(current === anchor),
        title: FRAME_ANCHOR_LABELS[anchor],
      },
      [element("span", { "aria-hidden": "true" })],
    ) as HTMLButtonElement;
    button.disabled = box === undefined || geometry === undefined || visual === undefined;
    button.addEventListener("click", () => {
      if (!box || !geometry || button.disabled || (managed && current === anchor)) return;

      handlers.run({
        op: "setBlockStyle",
        slide: state.selection.slide,
        block,
        property: "inset",
        value: insetOf(alignedFrame(box.rect, geometry.safe, anchor), geometry.safe),
      });
    });
    return button;
  });

  return element("div", { class: "slidx-frame-position" }, [
    element("div", { class: "slidx-frame-position-head" }, [
      element("span", { class: "slidx-frame-position-state", "data-pinned": String(managed) }, [
        managed ? "Pinned to safe area" : "Following layout",
      ]),
      reset,
    ]),
    element(
      "div",
      { class: "slidx-frame-anchors", role: "group", "aria-label": "Block safe-area position" },
      choices,
    ),
    element("p", { class: "slidx-frame-position-hint" }, [
      "Pin preserves this size. Layout position returns the block to its region.",
    ]),
  ]);
}

function regionControl(
  state: EditorState,
  block: number,
  box: BlockBox | undefined,
  geometry: SlideGeometry | undefined,
  handlers: BlockInspectorHandlers,
): HTMLElement {
  const names = regionsOf(state, geometry);
  if (names.length === 0) {
    return element("p", { class: "slidx-hint" }, ["This layout has one inherited region."]);
  }

  const current = box?.region ?? names[0];
  return element(
    "div",
    { class: "slidx-region-choices", role: "group", "aria-label": "Block region" },
    names.map((name) => {
      const selected = name === current;
      const button = element(
        "button",
        {
          type: "button",
          class: "slidx-region-choice",
          "data-region": name,
          "aria-pressed": String(selected),
        },
        [element("span", { "aria-hidden": "true" }, [name.slice(0, 2).toUpperCase()]), name],
      ) as HTMLButtonElement;
      button.addEventListener("click", () => {
        if (!selected) {
          handlers.run({
            op: "moveBlock",
            slide: state.selection.slide,
            block,
            to: block,
            region: name,
          });
        }
      });
      return button;
    }),
  );
}

function regionsOf(state: EditorState, geometry: SlideGeometry | undefined): string[] {
  const slide = state.slides[state.selection.slide];
  const inherited = scalar(slide?.frontmatter?.layout) ?? "full";
  const layout = state.layouts.find((choice) => choice.id === (slide?.style.layout ?? inherited));
  const declared = layout?.areas.flatMap((row) => row.split(/\s+/)) ?? [];
  const measured = geometry?.regions.map((region) => region.name) ?? [];
  return [...new Set([...declared, ...measured].filter((name) => name.length > 0 && name !== "."))];
}

function widthControl(
  state: EditorState,
  block: number,
  box: BlockBox | undefined,
  handlers: BlockInspectorHandlers,
): HTMLElement {
  const current = box?.width ?? "full";
  return element(
    "div",
    { class: "slidx-width-choices", role: "group", "aria-label": "Block width" },
    WIDTHS.map((width) => {
      const selected = current === width.name;
      const bar = element("span", { class: "slidx-width-bar" });
      bar.style.width = `${width.share * 100}%`;
      const button = element(
        "button",
        {
          type: "button",
          class: "slidx-width-choice",
          "data-width": width.name,
          "aria-label": width.name,
          "aria-pressed": String(selected),
          title: width.name,
        },
        [element("span", { class: "slidx-width-track", "aria-hidden": "true" }, [bar])],
      ) as HTMLButtonElement;
      button.addEventListener("click", () => {
        if (!selected) {
          handlers.run({
            op: "setBlockWidth",
            slide: state.selection.slide,
            block,
            width: width.name,
          });
        }
      });
      return button;
    }),
  );
}

function colorControl(
  state: EditorState,
  block: number,
  visual: BlockVisual | undefined,
  handlers: BlockInspectorHandlers,
): HTMLElement {
  const palette = new Map(visual?.palette.map((color) => [color.name, color]));
  const choices: Array<{ name: "theme" | ThemeColor["name"]; label: string }> = [
    { name: "theme", label: "Theme" },
    ...(visual?.palette ?? []).map(({ name, label }) => ({ name, label })),
  ];
  const buttons = element(
    "div",
    { class: "slidx-block-palette", role: "group", "aria-label": "Block theme color" },
    choices.map(({ name, label }) => {
      const selected = name === "theme" ? !visual?.managedColor : visual?.themeColor === name;
      const button = element(
        "button",
        {
          type: "button",
          class: "slidx-block-color-choice",
          "data-theme-color": name,
          "aria-label": name === "theme" ? "Use inherited theme color" : `Use ${label} theme color`,
          "aria-pressed": String(selected),
        },
        [colorSwatch(name, palette), element("span", {}, [label])],
      ) as HTMLButtonElement;
      button.disabled = visual === undefined;
      button.addEventListener("click", () => {
        if (selected) return;
        handlers.run({
          op: "setBlockStyle",
          slide: state.selection.slide,
          block,
          property: "color",
          ...(name === "theme" ? {} : { value: `var(--slidx-color-${name})` }),
        });
      });
      return button;
    }),
  );

  const picker = element("input", {
    type: "color",
    class: "slidx-block-color-input",
    "aria-label": "Custom block color",
  }) as HTMLInputElement;
  picker.value = hexColor(visual?.color ?? "#000000");
  picker.disabled = visual === undefined;

  const value = element("code", { class: "slidx-block-color-value" }, [picker.value]);

  picker.addEventListener("change", () => {
    value.textContent = picker.value;
    handlers.run({
      op: "setBlockStyle",
      slide: state.selection.slide,
      block,
      property: "color",
      value: picker.value,
    });
  });

  const custom = element(
    "details",
    {
      class: "slidx-block-color-custom",
      open: visual?.managedColor === true && visual.themeColor === undefined,
    },
    [
      element("summary", {}, ["Custom color"]),
      element("div", { class: "slidx-block-color" }, [picker, value]),
    ],
  );

  return element("div", { class: "slidx-block-appearance" }, [
    buttons,
    element("p", { class: "slidx-block-color-hint" }, [
      "Theme colors adapt with the deck. A custom color stays fixed.",
    ]),
    custom,
  ]);
}

function colorSwatch(
  name: "theme" | ThemeColor["name"],
  palette: ReadonlyMap<ThemeColor["name"], ThemeColor>,
): HTMLElement {
  const swatch = element("span", { class: "slidx-block-color-swatch", "aria-hidden": "true" });
  if (name === "theme") {
    for (const role of ["text", "accent"] as const) {
      const half = element("span");
      half.style.backgroundColor = palette.get(role)?.color ?? "transparent";
      swatch.append(half);
    }
  } else {
    swatch.style.backgroundColor = palette.get(name)?.color ?? "transparent";
  }
  return swatch;
}

function attributeControl(
  state: EditorState,
  block: number,
  current: BlockAttributes | undefined,
  handlers: BlockInspectorHandlers,
): HTMLElement {
  const key = element("input", { type: "text", placeholder: "result", value: current?.key ?? "" });
  const classes = element("input", {
    type: "text",
    placeholder: "accent",
    value: current?.classes?.join(" ") ?? "",
  });
  const properties = element("textarea", {
    rows: 3,
    placeholder: "tone=strong",
    "aria-label": "Block properties",
  });
  properties.value = showProperties(current?.properties);

  return element("div", { class: "slidx-block-identity" }, [
    field("Name", key),
    field("Classes", classes),
    field("Properties", properties),
    action("Apply identity", "slidx-block-attributes", () =>
      handlers.run({
        op: "setBlockAttributes",
        slide: state.selection.slide,
        block,
        attributes: attributes(key.value, classes.value, properties.value),
      }),
    ),
  ]);
}

function attributes(key: string, classes: string, properties: string): BlockAttributes {
  return {
    key: key.trim() || undefined,
    classes: classes.split(/\s+/).filter(Boolean),
    properties: Object.fromEntries(
      properties
        .split("\n")
        .map((line) => {
          const at = line.indexOf("=");
          return at === -1 ? undefined : [line.slice(0, at).trim(), line.slice(at + 1).trim()];
        })
        .filter((entry): entry is [string, string] => Boolean(entry?.[0])),
    ),
  };
}

function showProperties(properties: Record<string, string> | undefined): string {
  return Object.entries(properties ?? {})
    .map(([name, value]) => `${name}=${value}`)
    .join("\n");
}

function section(name: string, children: Node[]): HTMLElement {
  return element("section", { class: "slidx-inspector-section" }, [
    element("h4", {}, [name]),
    ...children,
  ]);
}

function action(name: string, className: string, act: () => void): HTMLButtonElement {
  const button = element("button", { type: "button", class: className }, [
    name,
  ]) as HTMLButtonElement;
  button.addEventListener("click", act);
  return button;
}

function scalar(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function truncate(value: string, length: number): string {
  return value.length > length ? `${value.slice(0, length - 1)}…` : value;
}

function group(name: string, children: Node[]): HTMLElement {
  return element("div", { class: "slidx-group", "data-group": name.toLowerCase() }, [
    element("h3", {}, [name]),
    ...children,
  ]);
}
