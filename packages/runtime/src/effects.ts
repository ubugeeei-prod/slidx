/**
 * Loads the one stylesheet that gives step attributes their visual meaning.
 *
 * Kept beside the runtime module, so `import.meta.url` is the whole addressing
 * contract: a plugin may move both under any base path without teaching this
 * package about that path. The stylesheet is shared and cacheable rather than
 * copied into every staged slide, while a slide with one stop asks for neither
 * this module nor the stylesheet.
 *
 * Failure is an answer rather than an exception. The stage can still update
 * text and URLs, and without the CSS every element stays visible — the useful
 * degradation for a room where an asset did not arrive.
 */

const loads = new WeakMap<Document, Promise<boolean>>();

/** Fetches and applies the step stylesheet once for this document. */
export function loadEffects(
  document: Document,
  href = new URL("./effects.css", import.meta.url).href,
): Promise<boolean> {
  const held = loads.get(document);
  if (held) return held;

  const loading = new Promise<boolean>((resolve) => {
    const link = document.createElement("link");
    link.rel = "stylesheet";
    link.href = href;
    link.dataset.slidxEffects = "";

    link.addEventListener("load", () => resolve(true), { once: true });
    link.addEventListener(
      "error",
      () => {
        link.remove();
        loads.delete(document);
        resolve(false);
      },
      { once: true },
    );

    document.head.append(link);
  });

  loads.set(document, loading);
  return loading;
}
