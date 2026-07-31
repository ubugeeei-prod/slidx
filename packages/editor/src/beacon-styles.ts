/** The outline and name drawn over a block somebody else has selected. */

export const BEACON_STYLESHEET = `
.slidx-beacons {
  position: fixed;
  inset: 0;
  z-index: 6;
  pointer-events: none;
}

.slidx-beacon {
  position: fixed;
  outline: 2px solid var(--slidx-beacon-ink);
  outline-offset: 2px;
  border-radius: 2px;
}

.slidx-beacon-name {
  position: absolute;
  left: -2px;
  bottom: calc(100% + 4px);
  padding: 1px 6px;
  border-radius: 2px;
  background: var(--slidx-beacon-ink);
  color: #ffffff;
  font-family: var(--slidx-e-font-sans);
  font-size: 11px;
  line-height: 16px;
  white-space: nowrap;
}
`;

export function applyBeaconStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-beacons]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-beacons", "");
  style.textContent = BEACON_STYLESHEET;
  document.head.append(style);
}
