/**
 * Writing one element's state onto the DOM.
 *
 * Split from the stage because it answers a different question: the stage
 * decides *which* frame is showing, this decides what showing a frame means
 * for a single element.
 *
 * Everything here is absolute rather than incremental. A frame is a complete
 * state, so "no content override" means *restore the markup's text*, not
 * "leave whatever the last step wrote". Getting that wrong is what turns a
 * presenter stepping backwards into a slide that never recovers.
 */

/** Reserved dataset keys the pipeline owns, which are not author properties. */
const RESERVED_DATASET = new Set(["slidxStep", "slidxMark", "slidxStaged", "slidxHidden"]);

/**
 * Writes the text a stop calls for, restoring the markup's own when it calls
 * for none.
 *
 * `textContent` rather than `innerHTML`: a patch replaces a *value*, and
 * letting a timeline inject markup would turn deck data into a script vector.
 */
export function setContent(
  element: HTMLElement,
  original: Map<HTMLElement, string>,
  content: string | undefined,
): void {
  const next = content ?? original.get(element) ?? "";
  if (element.textContent !== next) element.textContent = next;
}

/**
 * Reconciles data properties against the frame.
 *
 * Properties the frame does not mention are removed rather than left in place,
 * because a frame is a complete state: stepping back past the stop that set a
 * colour has to take the colour away again.
 */
export function setProperties(
  element: HTMLElement,
  properties: Record<string, string> | undefined,
): void {
  const wanted = properties ?? {};

  for (const name of Object.keys(element.dataset)) {
    if (!name.startsWith("slidx")) continue;
    const key = propertyName(name);
    if (key && !(key in wanted)) delete element.dataset[name];
  }

  for (const [key, value] of Object.entries(wanted)) {
    element.setAttribute(`data-slidx-${key}`, value);
  }
}

function propertyName(datasetKey: string): string | null {
  if (RESERVED_DATASET.has(datasetKey) || datasetKey.startsWith("slidxEffect")) return null;

  const rest = datasetKey.slice("slidx".length);
  return rest ? rest[0]!.toLowerCase() + rest.slice(1) : null;
}
