/** Theme-aware appearance controls for words selected on the rendered slide. */

import type { BlockSpans, MarkSpans } from "./client";
import { element, field } from "./dom";
import type { ByteSpan, EditOp, MarkAttributes } from "./operations";
import { locateSelection } from "./selection";
import type { EditorState } from "./session";

export interface TextInspectorHandlers {
  run(op: EditOp): void;
}

export interface TextInspectorOptions {
  bodyOf(slide: number): string;
  blocksOf?(slide: number): readonly BlockSpans[];
}

export type TextTone = "theme" | "accent" | "muted" | "danger" | "success";
export type TextWeight = "regular" | "bold";
export type TextFace = "theme" | "code";

export const TEXT_TONES: Array<{ value: TextTone; label: string }> = [
  { value: "theme", label: "Theme" },
  { value: "accent", label: "Accent" },
  { value: "muted", label: "Muted" },
  { value: "danger", label: "Danger" },
  { value: "success", label: "Success" },
];

export interface TextStyleTarget {
  slide: number;
  text: string;
  range: ByteSpan;
  /** Minus one when the selected words are not wrapped yet. */
  mark: number;
  attributes: MarkAttributes;
}

export type TextStyleResolution =
  | { target: TextStyleTarget }
  | { problem: "empty" | "not-found" | "overlap"; message: string };

/** Resolves the same source-backed text target for every styling surface. */
export function resolveTextStyle(
  state: EditorState,
  options: TextInspectorOptions,
): TextStyleResolution {
  const selected = state.selection.text ?? "";
  if (selected.length === 0) {
    return { problem: "empty", message: "Select words in the slide to style them." };
  }

  const located = locateSelection(options.bodyOf(state.selection.slide), selected, 0);
  const range = state.selection.range ?? ("problem" in located ? undefined : located.range);
  if (range === undefined) {
    return {
      problem: "not-found",
      message: `“${selected}” is written differently in the Markdown, so it cannot be addressed yet.`,
    };
  }

  const marks = (options.blocksOf?.(state.selection.slide) ?? []).flatMap(
    (block) => block.marks ?? [],
  );
  const exact = marks.findIndex((mark) => same(mark.words, range));
  if (exact === -1 && marks.some((mark) => intersects(mark.words, range))) {
    return {
      problem: "overlap",
      message: "Select the whole styled phrase to change it without nesting marks.",
    };
  }

  return {
    target: {
      slide: state.selection.slide,
      text: selected.trim(),
      range,
      mark: exact,
      attributes: attributesOf(exact === -1 ? undefined : marks[exact]),
    },
  };
}

/** The one semantic operation both the quick bar and inspector produce. */
export function textStyleOperation(target: TextStyleTarget, attributes: MarkAttributes): EditOp {
  return {
    op: target.mark === -1 ? "addMark" : "setMark",
    slide: target.slide,
    ...(target.mark === -1 ? { range: target.range } : { mark: target.mark }),
    attributes,
  } as EditOp;
}

/**
 * Turns a source selection into direct, reviewable style operations.
 *
 * The common controls only own their own small vocabulary. An unknown class or
 * property survives every preset click, so a theme extension and this editor
 * can style the same phrase without taking turns overwriting one another.
 */
export function textPanel(
  state: EditorState,
  handlers: TextInspectorHandlers,
  options: TextInspectorOptions,
): HTMLElement {
  const resolved = resolveTextStyle(state, options);
  if ("problem" in resolved) return hint(resolved.message);

  const { target } = resolved;
  const current = target.attributes;
  const locked = state.writing || state.canEdit === false;
  const commit = (attributes: MarkAttributes) =>
    handlers.run(textStyleOperation(target, attributes));

  return group([
    element("div", { class: "slidx-text-context" }, [
      element("span", { class: "slidx-text-context-mark", "aria-hidden": "true" }, ["Aa"]),
      element("div", {}, [
        element("span", { class: "slidx-text-context-label" }, ["Selected words"]),
        element("p", { class: "slidx-selected" }, [target.text]),
      ]),
    ]),
    section("Tone", [toneControl(current, locked, commit)]),
    section("Emphasis", [
      segmented(
        "Text weight",
        [
          { value: "regular", label: "Regular", mark: "A" },
          { value: "bold", label: "Bold", mark: "B" },
        ],
        textWeightOf(current),
        locked,
        (value) => commit(withTextWeight(current, value)),
      ),
    ]),
    section("Typeface", [
      segmented(
        "Text typeface",
        [
          { value: "theme", label: "Theme", mark: "Aa" },
          { value: "code", label: "Mono", mark: "<>" },
        ],
        textFaceOf(current),
        locked,
        (value) => commit(withTextFace(current, value)),
      ),
    ]),
    advanced(target, current, locked, handlers),
  ]);
}

function toneControl(
  current: MarkAttributes,
  locked: boolean,
  commit: (attributes: MarkAttributes) => void,
): HTMLElement {
  const selected = textToneOf(current);

  return element(
    "div",
    { class: "slidx-text-tones", role: "group", "aria-label": "Text tone" },
    TEXT_TONES.map(({ value, label }) => {
      const button = element(
        "button",
        {
          type: "button",
          class: "slidx-text-tone",
          "data-tone": value,
          "aria-pressed": String(selected === value),
          disabled: locked,
        },
        [element("span", { class: "slidx-text-tone-swatch", "aria-hidden": "true" }), label],
      );
      button.addEventListener("click", () => {
        if (selected !== value) commit(withTextTone(current, value));
      });
      return button;
    }),
  );
}

function segmented<T extends string>(
  label: string,
  choices: Array<{ value: T; label: string; mark: string }>,
  selected: T | undefined,
  locked: boolean,
  commit: (value: T) => void,
): HTMLElement {
  return element(
    "div",
    { class: "slidx-text-segments", role: "group", "aria-label": label },
    choices.map(({ value, label: name, mark }) => {
      const button = element(
        "button",
        {
          type: "button",
          class: "slidx-text-segment",
          "data-value": value,
          "aria-pressed": String(selected === value),
          disabled: locked,
        },
        [element("span", { "aria-hidden": "true" }, [mark]), name],
      );
      button.addEventListener("click", () => {
        if (selected !== value) commit(value);
      });
      return button;
    }),
  );
}

function advanced(
  target: TextStyleTarget,
  current: MarkAttributes,
  locked: boolean,
  handlers: TextInspectorHandlers,
): HTMLElement {
  const classes = element("input", {
    type: "text",
    placeholder: "accent",
    value: current.classes?.join(" ") ?? "",
    disabled: locked,
  });
  const key = element("input", {
    type: "text",
    placeholder: "result",
    value: current.key ?? "",
    disabled: locked,
  });
  const properties = element("textarea", {
    rows: 3,
    placeholder: "color=danger\nweight=bold",
    "aria-label": "Style properties",
    disabled: locked,
  });
  properties.value = showProperties(current.properties);

  const apply = element("button", { type: "button", class: "slidx-add", disabled: locked }, [
    target.mark === -1 ? "Apply" : "Update",
  ]);
  apply.addEventListener("click", () =>
    handlers.run(
      textStyleOperation(target, attributes(key.value, classes.value, properties.value)),
    ),
  );

  const remove = element(
    "button",
    {
      type: "button",
      class: "slidx-remove-mark",
      disabled: locked || target.mark === -1,
    },
    ["Remove mark"],
  );
  remove.addEventListener("click", () => {
    if (target.mark !== -1) {
      handlers.run({ op: "removeMark", slide: target.slide, mark: target.mark });
    }
  });

  return element("details", { class: "slidx-text-advanced" }, [
    element("summary", {}, [
      element("span", {}, ["Advanced"]),
      element("span", { class: "slidx-text-advanced-hint" }, ["Name · classes · properties"]),
    ]),
    element("div", { class: "slidx-text-advanced-body" }, [
      field("Classes", classes),
      field("Name", key),
      field("Properties", properties),
      element("div", { class: "slidx-text-advanced-actions" }, [apply, remove]),
    ]),
  ]);
}

export function withTextTone(current: MarkAttributes, tone: TextTone): MarkAttributes {
  const classes = [...(current.classes ?? [])].filter(
    (name) => name !== "accent" && name !== "muted",
  );
  const properties = { ...current.properties };
  delete properties.color;

  if (tone === "accent" || tone === "muted") classes.push(tone);
  if (tone === "danger" || tone === "success") properties.color = tone;
  return tidy({ ...current, classes, properties });
}

export function withTextWeight(current: MarkAttributes, weight: TextWeight): MarkAttributes {
  const properties = { ...current.properties };
  if (weight === "bold") properties.weight = "bold";
  else delete properties.weight;
  return tidy({ ...current, properties });
}

/** Flips the common emphasis without disturbing a theme extension. */
export function toggleTextWeight(current: MarkAttributes): MarkAttributes {
  return withTextWeight(current, textWeightOf(current) === "bold" ? "regular" : "bold");
}

export function withTextFace(current: MarkAttributes, face: TextFace): MarkAttributes {
  const classes = [...(current.classes ?? [])].filter((name) => name !== "code");
  if (face === "code") classes.push("code");
  return tidy({ ...current, classes });
}

/** Flips the common typeface without disturbing another class. */
export function toggleTextFace(current: MarkAttributes): MarkAttributes {
  return withTextFace(current, textFaceOf(current) === "code" ? "theme" : "code");
}

export function textToneOf(current: MarkAttributes): TextTone | undefined {
  const color = current.properties?.color;
  if (color === "danger" || color === "success") return color;
  if (current.classes?.includes("accent")) return "accent";
  if (current.classes?.includes("muted")) return "muted";
  if (color === undefined) return "theme";
  return undefined;
}

export function textWeightOf(current: MarkAttributes): TextWeight | undefined {
  const weight = current.properties?.weight;
  if (weight === "bold") return "bold";
  return weight === undefined ? "regular" : undefined;
}

export function textFaceOf(current: MarkAttributes): TextFace {
  return current.classes?.includes("code") ? "code" : "theme";
}

function attributesOf(mark: MarkSpans | undefined): MarkAttributes {
  return tidy({
    key: mark?.key,
    classes: [...(mark?.classes ?? [])],
    properties: { ...mark?.properties },
  });
}

function tidy(value: MarkAttributes): MarkAttributes {
  return {
    key: value.key,
    classes: value.classes?.filter(Boolean) ?? [],
    properties: Object.fromEntries(Object.entries(value.properties ?? {}).filter(([name]) => name)),
  };
}

function attributes(key: string, classes: string, properties: string): MarkAttributes {
  return {
    key: key.trim() || undefined,
    classes: classes.split(/\s+/).filter(Boolean),
    properties: parseProperties(properties),
  };
}

function parseProperties(source: string): Record<string, string> {
  return Object.fromEntries(
    source
      .split("\n")
      .map((line) => {
        const at = line.indexOf("=");
        return at === -1 ? undefined : [line.slice(0, at).trim(), line.slice(at + 1).trim()];
      })
      .filter((entry): entry is [string, string] => Boolean(entry?.[0])),
  );
}

function showProperties(properties: Record<string, string> | undefined): string {
  return Object.entries(properties ?? {})
    .map(([name, value]) => `${name}=${value}`)
    .join("\n");
}

function same(left: ByteSpan, right: ByteSpan): boolean {
  return left.start === right.start && left.end === right.end;
}

function intersects(left: ByteSpan, right: ByteSpan): boolean {
  return left.start < right.end && right.start < left.end;
}

function section(name: string, children: Node[]): HTMLElement {
  return element("section", { class: "slidx-inspector-section slidx-text-section" }, [
    element("h4", {}, [name]),
    ...children,
  ]);
}

function hint(message: string): HTMLElement {
  return group([element("p", { class: "slidx-hint" }, [message])]);
}

function group(children: Node[]): HTMLElement {
  return element("div", { class: "slidx-group", "data-group": "selection" }, [
    element("h3", {}, ["Text"]),
    ...children,
  ]);
}
