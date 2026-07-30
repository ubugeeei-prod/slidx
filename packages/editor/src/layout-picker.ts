/**
 * A pipeline-provided visual layout picker.
 *
 * Layouts are data, not names understood by the editor: the compiler supplies
 * their grid areas and tracks, and this surface draws those exact definitions.
 * Choosing one writes only `--slidx-layout` through `setStyle`, so the decision
 * round-trips through the slide's Markdown `<style>` block.
 */

import type { LayoutChoice, SlideSummary } from "./client";
import { element, field } from "./dom";
import { applyLayoutPickerStyles } from "./layout-picker-styles";
import type { EditOp } from "./operations";
import type { EditorState } from "./session";

export function layoutField(
  state: EditorState,
  slide: SlideSummary,
  index: number,
  run: (op: EditOp) => void,
): HTMLElement {
  const explicit = slide.style.layout;
  const inherited = scalar(slide.frontmatter?.layout) ?? "full";
  const known = state.layouts.find((layout) => layout.id === inherited);
  const fallback =
    known ??
    ({
      id: inherited,
      summary: "Inherited from this slide's frontmatter.",
      areas: ["body"],
      columns: "1fr",
      rows: "1fr",
    } satisfies LayoutChoice);
  const choices = [...state.layouts];

  // A package or a newer pipeline may have written a layout this editor did
  // not receive. It stays visible and selectable rather than disappearing.
  if (explicit && !choices.some((layout) => layout.id === explicit)) {
    choices.push({
      id: explicit,
      summary: "A custom layout kept from the Markdown source.",
      areas: ["body"],
      columns: "1fr",
      rows: "1fr",
    });
  }

  const picker = element("div", {
    class: "slidx-layout-picker",
    role: "group",
    "aria-label": "Slide layout",
  });
  applyLayoutPickerStyles(picker.ownerDocument);
  picker.append(
    layoutButton("", `Inherited · ${inherited}`, fallback, explicit === undefined, () =>
      run({ op: "setStyle", slide: index, property: "layout" }),
    ),
    ...choices.map((layout) =>
      layoutButton(layout.id, layout.id, layout, explicit === layout.id, () =>
        run({
          op: "setStyle",
          slide: index,
          property: "layout",
          value: layout.id,
        }),
      ),
    ),
  );

  const result = field("Layout", picker);
  result.classList.add("slidx-layout-field");
  return result;
}

function layoutButton(
  value: string,
  label: string,
  layout: LayoutChoice,
  selected: boolean,
  choose: () => void,
): HTMLButtonElement {
  const preview = element("span", { class: "slidx-layout-preview", "aria-hidden": "true" });
  preview.style.gridTemplateAreas = layout.areas.map((row) => `"${row}"`).join(" ");
  preview.style.gridTemplateColumns = layout.columns;
  preview.style.gridTemplateRows = layout.rows;

  for (const area of uniqueAreas(layout.areas)) {
    const region = element("span", { class: "slidx-layout-region" });
    region.style.gridArea = area;
    preview.append(region);
  }

  const button = element(
    "button",
    {
      type: "button",
      class: "slidx-layout-choice",
      "data-layout": value,
      "aria-pressed": String(selected),
      title: layout.summary,
    },
    [preview, element("span", { class: "slidx-layout-name" }, [label])],
  );
  button.addEventListener("click", () => {
    if (!selected) choose();
  });
  return button;
}

function uniqueAreas(rows: string[]): string[] {
  return [
    ...new Set(
      rows.flatMap((row) => row.split(/\s+/)).filter((area) => area.length > 0 && area !== "."),
    ),
  ];
}

function scalar(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}
