/**
 * The selected block's colour as the real deck rendered it.
 *
 * A native colour input wants six-digit hex even though `getComputedStyle`
 * returns `rgb()`. Keeping the conversion at this boundary means the editor
 * writes exactly the colour the author picked, while Reset can remove the
 * managed declaration and return the block to its theme.
 */

export interface BlockVisual {
  color: string;
  managedColor: boolean;
}

export function visualOf(frame: HTMLIFrameElement, selected: number): BlockVisual | undefined {
  const block = frame.contentDocument?.querySelector<HTMLElement>(
    `[data-slidx-block="${selected}"]`,
  );
  const view = frame.contentWindow;
  if (!block || !view) return undefined;

  return {
    color: view.getComputedStyle(block).color,
    managedColor: block.hasAttribute("data-slidx-element-color"),
  };
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
