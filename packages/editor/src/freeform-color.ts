/**
 * The selected block's colour as the real deck rendered it.
 *
 * A native colour input wants six-digit hex even though `getComputedStyle`
 * returns `rgb()`. Keeping the conversion at this boundary means the editor
 * writes exactly the colour the author picked, while Reset can remove the
 * managed declaration and return the block to its theme.
 */

export const THEME_COLOR_NAMES = ["text", "heading", "muted", "accent"] as const;

export type ThemeColorName = (typeof THEME_COLOR_NAMES)[number];

export interface ThemeColor {
  name: ThemeColorName;
  label: string;
  color: string;
}

export interface BlockVisual {
  color: string;
  managedColor: boolean;
  /** True when a visual gesture pinned this block to the slide's safe area. */
  managedFrame?: boolean | undefined;
  /** The exact value authored in the managed block property. */
  managedValue?: string | undefined;
  /** Present only when the authored value names one adaptive theme role. */
  themeColor?: ThemeColorName | undefined;
  palette: ThemeColor[];
}

const THEME_COLOR_LABELS: Record<ThemeColorName, string> = {
  text: "Text",
  heading: "Heading",
  muted: "Muted",
  accent: "Accent",
};

export function visualOf(frame: HTMLIFrameElement, selected: number): BlockVisual | undefined {
  const page = frame.contentDocument;
  const block = page?.querySelector<HTMLElement>(`[data-slidx-block="${selected}"]`);
  const view = frame.contentWindow;
  if (!page || !block || !view) return undefined;

  const root = view.getComputedStyle(page.documentElement);
  const palette = THEME_COLOR_NAMES.map((name) => ({
    name,
    label: THEME_COLOR_LABELS[name],
    color: root.getPropertyValue(`--slidx-color-${name}`).trim(),
  }));
  const managedValue = authoredColor(block);
  const themeColor = themeName(managedValue);

  return {
    color: view.getComputedStyle(block).color,
    managedColor: block.hasAttribute("data-slidx-element-color"),
    managedFrame: block.hasAttribute("data-slidx-freeform-frame"),
    managedValue,
    themeColor,
    palette,
  };
}

/** The value before CSS resolves it, so a fixed hex never impersonates a theme role. */
function authoredColor(block: HTMLElement): string | undefined {
  const binding = block.style.getPropertyValue("--slidx-element-color").trim();
  const property = binding.match(/^var\(\s*(--[-\w]+)\s*\)$/)?.[1];
  const value = property
    ? block.closest<HTMLElement>(".slidx-slide")?.style.getPropertyValue(property).trim()
    : "";
  return value || undefined;
}

function themeName(value: string | undefined): ThemeColorName | undefined {
  const name = value?.match(/^var\(\s*--slidx-color-(text|heading|muted|accent)\s*\)$/)?.[1];
  return THEME_COLOR_NAMES.includes(name as ThemeColorName) ? (name as ThemeColorName) : undefined;
}

export function hexColor(value: string): string {
  const hex = value.trim().match(/^#([\da-f]{3}|[\da-f]{6})$/i)?.[1];
  if (hex) {
    return hex.length === 3
      ? `#${hex[0]}${hex[0]}${hex[1]}${hex[1]}${hex[2]}${hex[2]}`.toLowerCase()
      : `#${hex}`.toLowerCase();
  }

  const channels = value.match(/rgba?\(\s*(\d+(?:\.\d+)?)\D+(\d+(?:\.\d+)?)\D+(\d+(?:\.\d+)?)/i);
  if (!channels) return "#000000";

  return `#${channels
    .slice(1, 4)
    .map((channel) =>
      Math.max(0, Math.min(255, Math.round(Number(channel))))
        .toString(16)
        .padStart(2, "0"),
    )
    .join("")}`;
}
