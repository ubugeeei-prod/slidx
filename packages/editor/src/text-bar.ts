/** High-frequency text appearance controls beside the selected words. */

import { element } from "./dom";
import type { EditOp, MarkAttributes } from "./operations";
import type { Surface } from "./outline";
import type { EditorState } from "./session";
import { applyTextBarStyles } from "./text-bar-styles";
import {
  resolveTextStyle,
  TEXT_TONES,
  textFaceOf,
  textStyleOperation,
  textToneOf,
  textWeightOf,
  toggleTextFace,
  toggleTextWeight,
  withTextTone,
  type TextFace,
  type TextInspectorOptions,
  type TextStyleTarget,
  type TextTone,
  type TextWeight,
} from "./text-inspector";

export interface TextBarHandlers {
  run(op: EditOp): void;
  done(): void;
}

interface Choice<T extends string> {
  button: HTMLButtonElement;
  value: T;
}

/**
 * Keeps the three text decisions used most often in the canvas header.
 *
 * The inspector remains the complete surface. This bar deliberately delegates
 * target resolution and attribute changes to the same functions, so a fast
 * click cannot become a second interpretation of the selected Markdown.
 */
export function createTextBar(handlers: TextBarHandlers, options: TextInspectorOptions): Surface {
  const toneChoices: Array<Choice<TextTone>> = TEXT_TONES.map(({ value, label }) => {
    const button = element(
      "button",
      {
        type: "button",
        class: "slidx-text-bar-tone",
        "data-tone": value,
        "aria-label": `${label} text tone`,
        title: `${label} text tone`,
      },
      [element("span", { class: "slidx-text-bar-swatch", "aria-hidden": "true" })],
    ) as HTMLButtonElement;

    return { button, value };
  });
  const bold = toggle(
    "bold",
    "B",
    "Bold selected text",
    "Control+B Meta+B",
    "Bold selected text (⌘/Ctrl B)",
  );
  const mono = toggle("code", "<>", "Use mono typeface");
  const done = element(
    "button",
    { type: "button", class: "slidx-text-bar-done", "aria-label": "Finish text styling" },
    ["Done"],
  ) as HTMLButtonElement;
  const root = element(
    "div",
    {
      class: "slidx-text-bar",
      role: "toolbar",
      "aria-label": "Selected text styles",
      hidden: true,
    },
    [
      element(
        "div",
        { class: "slidx-text-bar-tones", role: "group", "aria-label": "Text tone" },
        toneChoices.map(({ button }) => button),
      ),
      element("div", { class: "slidx-text-bar-actions" }, [bold.button, mono.button, done]),
    ],
  );
  applyTextBarStyles(root.ownerDocument);

  let latest: EditorState | undefined;
  let target: TextStyleTarget | undefined;

  const commit = (attributes: MarkAttributes) => {
    if (!target || latest?.writing || latest?.canEdit === false) return;
    handlers.run(textStyleOperation(target, attributes));
  };

  for (const choice of toneChoices) {
    choice.button.addEventListener("click", () => {
      if (target && textToneOf(target.attributes) !== choice.value) {
        commit(withTextTone(target.attributes, choice.value));
      }
    });
  }
  bold.button.addEventListener("click", () => {
    if (target) commit(toggleTextWeight(target.attributes));
  });
  mono.button.addEventListener("click", () => {
    if (target) commit(toggleTextFace(target.attributes));
  });
  done.addEventListener("click", () => handlers.done());

  const show = (visible: boolean) => {
    root.hidden = !visible;
    const header = root.parentElement;
    if (!header) return;
    header.setAttribute("data-text-tools", String(visible));
    const title = header.querySelector("h2");
    if (title) title.textContent = visible ? "Text" : "Slide";
  };

  return {
    root,
    render(state) {
      latest = state;
      const resolved = resolveTextStyle(state, options);
      target = "target" in resolved ? resolved.target : undefined;
      show(target !== undefined);
      if (!target) return;

      const locked = state.writing || state.canEdit === false;
      for (const choice of toneChoices) {
        choice.button.disabled = locked;
        choice.button.setAttribute(
          "aria-pressed",
          String(textToneOf(target.attributes) === choice.value),
        );
      }
      updateToggle(bold, textWeightOf(target.attributes) === "bold", locked);
      updateToggle(mono, textFaceOf(target.attributes) === "code", locked);
    },
    destroy() {
      show(false);
    },
  };
}

function toggle<T extends TextWeight | TextFace>(
  style: "bold" | "code",
  mark: string,
  label: string,
  keys?: string,
  title = label,
): Choice<T> {
  const button = element(
    "button",
    {
      type: "button",
      class: "slidx-text-bar-toggle",
      "data-style": style,
      "aria-label": label,
      "aria-keyshortcuts": keys,
      title,
    },
    [mark],
  ) as HTMLButtonElement;
  return { button, value: style as T };
}

function updateToggle(choice: Choice<string>, pressed: boolean, locked: boolean): void {
  choice.button.disabled = locked;
  choice.button.setAttribute("aria-pressed", String(pressed));
}
