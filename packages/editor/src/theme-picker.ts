/** A scheme-aware gallery built from the themes the active pipeline resolved. */

import type { ThemeChoice, ThemePaletteChoice } from "./client";
import { element } from "./dom";
import type { EditOp } from "./operations";
import type { EditorState } from "./session";
import { applyThemePickerStyles } from "./theme-picker-styles";

export type VisibleScheme = "light" | "dark";

export function themeField(
  state: EditorState,
  scheme: VisibleScheme,
  run: (op: EditOp) => void,
): HTMLElement {
  applyThemePickerStyles(document);

  const authored = scalar(state.slides[0]?.frontmatter?.theme);
  const current = state.themeLocked ? state.activeTheme : (authored ?? state.activeTheme);
  const offered = state.themes.some((theme) => theme.id === current);
  const picker = element(
    "div",
    { class: "slidx-theme-picker", role: "group", "aria-label": "Deck theme" },
    state.themes.map((theme) =>
      themeButton(theme, scheme, theme.id === current, state.themeLocked, () =>
        run({ op: "setField", slide: 0, key: "theme", value: theme.id }),
      ),
    ),
  );

  const notice = state.themeLocked
    ? "Theme is set by build configuration, so this deck cannot override it here."
    : authored && !offered
      ? `“${authored}” is not available in this pipeline. Choose a theme to repair it.`
      : undefined;

  return element("div", { class: "slidx-theme-field" }, [
    element("span", { class: "slidx-field-name" }, ["Theme"]),
    ...(notice ? [element("p", { class: "slidx-theme-notice", role: "status" }, [notice])] : []),
    picker,
  ]);
}

function themeButton(
  theme: ThemeChoice,
  scheme: VisibleScheme,
  selected: boolean,
  locked: boolean,
  choose: () => void,
): HTMLButtonElement {
  const palette = theme[scheme];
  const button = element(
    "button",
    {
      type: "button",
      class: "slidx-theme-choice",
      "data-theme": theme.id,
      "aria-label": `${theme.name}: ${theme.description}`,
      "aria-pressed": String(selected),
      title: theme.description,
      disabled: locked,
    },
    [
      miniature(theme, palette),
      element("span", { class: "slidx-theme-copy" }, [
        element("strong", {}, [theme.name]),
        element("span", {}, [theme.description]),
      ]),
    ],
  ) as HTMLButtonElement;

  button.addEventListener("click", () => {
    if (!selected && !locked) choose();
  });
  return button;
}

function miniature(theme: ThemeChoice, palette: ThemePaletteChoice): HTMLElement {
  const preview = element("span", { class: "slidx-theme-preview", "aria-hidden": "true" }, [
    element("span", { class: "slidx-theme-preview-accent" }),
    element("span", { class: "slidx-theme-preview-heading" }, ["Aa"]),
    element("span", { class: "slidx-theme-preview-line" }),
    element("span", { class: "slidx-theme-preview-line", "data-muted": "true" }),
    element("span", { class: "slidx-theme-preview-code" }, ["{ }"]),
  ]);
  preview.style.backgroundColor = palette.surface;
  preview.style.color = palette.text;
  preview.style.fontFamily = theme.fontSans;

  const accent = preview.querySelector<HTMLElement>(".slidx-theme-preview-accent")!;
  accent.style.backgroundColor = palette.accent;
  const heading = preview.querySelector<HTMLElement>(".slidx-theme-preview-heading")!;
  heading.style.color = palette.heading;
  const lines = preview.querySelectorAll<HTMLElement>(".slidx-theme-preview-line");
  lines[0]!.style.backgroundColor = palette.text;
  lines[1]!.style.backgroundColor = palette.muted;
  const code = preview.querySelector<HTMLElement>(".slidx-theme-preview-code")!;
  code.style.backgroundColor = palette.codeSurface;
  code.style.color = palette.codeText;
  code.style.fontFamily = theme.fontMono;

  return preview;
}

function scalar(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}
