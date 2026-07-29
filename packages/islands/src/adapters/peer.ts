/**
 * Loading a framework this package does not depend on.
 *
 * The frameworks are optional peer dependencies: declared, never installed.
 * That is not a packaging nicety — it is the reason a deck with no React in it
 * has no React anywhere near its install, its lockfile, or its bundle. The
 * cost is that `import("vue")` cannot be written literally here: the type
 * checker cannot resolve a module that is not installed, and a bundler would
 * try to resolve the specifier statically for every deck, including the ones
 * that genuinely do not have it.
 *
 * Routing the specifier through a variable keeps it opaque to both, so the
 * import is resolved by the deck's own build, where the framework either
 * exists or its absence is a real error. The cast that comes with that is the
 * only unchecked step in the package, which is why every adapter declares the
 * shape it needs structurally instead of importing types it cannot see.
 */

export async function importPeer<T>(specifier: string): Promise<T> {
  const loaded: unknown = await import(/* @vite-ignore */ specifier);
  return loaded as T;
}

/**
 * The component out of whatever the author's loader returned.
 *
 * `() => import("./Chart.vue")` resolves to a module namespace, not to a
 * component; a component written inline in the deck's setup file is already
 * one. Accepting both is what lets the option be written the way every
 * framework's own documentation writes it.
 */
export function resolveDefault(loaded: unknown): unknown {
  if (typeof loaded === "object" && loaded !== null && "default" in loaded) {
    return (loaded as { default: unknown }).default;
  }
  return loaded;
}

/**
 * What an adapter accepts as "the component".
 *
 * Deferred so the component travels with the framework rather than ahead of
 * it, and untyped because the loader is almost always `() => import(…)`, whose
 * result is a module namespace no adapter can describe in advance.
 */
export type ComponentLoader = () => unknown;
