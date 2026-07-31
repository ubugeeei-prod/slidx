/** Theme-aware text choices for the selection inspector. */

import { element } from "./dom";

interface Choice {
  label: string;
  value?: string;
  swatch?: string;
}

export interface TextControlOptions {
  properties: Record<string, string>;
  change(name: string, value: string | undefined): void;
}

const FONTS: Choice[] = [
  { label: "Default" },
  { label: "Sans", value: "sans" },
  { label: "Mono", value: "mono" },
];

const SIZES: Choice[] = [
  { label: "Default" },
  { label: "Caption", value: "caption" },
  { label: "Body", value: "body" },
  { label: "Code", value: "code" },
  { label: "H3", value: "heading-3" },
  { label: "H2", value: "heading-2" },
  { label: "H1", value: "heading-1" },
];

const COLORS: Choice[] = [
  { label: "Default" },
  { label: "Text", value: "text", swatch: "var(--slidx-e-text)" },
  { label: "Heading", value: "heading", swatch: "var(--slidx-e-text)" },
  { label: "Muted", value: "muted", swatch: "var(--slidx-e-muted)" },
  { label: "Accent", value: "accent", swatch: "var(--slidx-e-accent)" },
  { label: "Danger", value: "danger", swatch: "#b42318" },
  { label: "Success", value: "success", swatch: "#067647" },
];

/** Builds compact visual choices without hiding custom property values. */
export function createTextControls(options: TextControlOptions): HTMLElement {
  return element("div", { class: "slidx-text-controls", "aria-label": "Text appearance" }, [
    choices("Font", "font", FONTS, options),
    choices("Size", "size", SIZES, options),
    choices("Color", "color", COLORS, options),
  ]);
}

function choices(
  label: string,
  property: string,
  choices: Choice[],
  options: TextControlOptions,
): HTMLElement {
  const current = options.properties[property];
  const buttons = choices.map((choice) => {
    const selected =
      choice.value === current || (choice.value === undefined && current === undefined);
    const button = element(
      "button",
      {
        type: "button",
        class: `slidx-text-choice slidx-text-choice-${property}`,
        "data-property": property,
        "data-value": choice.value ?? "",
        "aria-label": `${label}: ${choice.label}`,
        "aria-pressed": selected,
        title: choice.label,
        ...(choice.swatch ? { style: `--slidx-text-swatch: ${choice.swatch}` } : {}),
      },
      property === "color"
        ? [element("span", { class: "slidx-text-swatch", "aria-hidden": "true" })]
        : [choice.label],
    );

    button.addEventListener("click", () => {
      for (const peer of buttons) peer.setAttribute("aria-pressed", String(peer === button));
      options.change(property, choice.value);
    });
    return button;
  });

  // An installed theme may define another semantic value. It stays in the
  // advanced field and no preset pretends it is selected.
  return element("div", { class: `slidx-text-control slidx-text-control-${property}` }, [
    element("span", { class: "slidx-text-control-name", id: `slidx-text-${property}` }, [label]),
    element(
      "div",
      { class: "slidx-text-choices", role: "group", "aria-labelledby": `slidx-text-${property}` },
      buttons,
    ),
  ]);
}
