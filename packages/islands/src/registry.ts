/**
 * Which frameworks this deck opted into.
 *
 * A deck registers the islands it uses, once, in its own setup module. That
 * registration is the opt-in: nothing is available by default, so a deck that
 * never writes `vue` never loads Vue and never mentions it in a bundle.
 *
 * The registry exists mainly so a *missing* island is diagnosable. `vue` in
 * frontmatter and `createVueIsland` never called is the overwhelmingly common
 * first mistake, and the difference between a useful five seconds and a lost
 * afternoon is whether the message names what is registered instead of only
 * what is not.
 */

import type { IslandDefinition } from "./contract";

export interface IslandRegistry {
  /** Adds a definition. A name registered twice keeps the later definition. */
  register(definition: IslandDefinition): void;
  /** The definition for `name`, or undefined. Surrounding whitespace is ignored. */
  lookup(name: string): IslandDefinition | undefined;
  has(name: string): boolean;
  /** Every registered name, sorted, so a message reads the same run to run. */
  names(): readonly string[];
}

export function createRegistry(definitions: Iterable<IslandDefinition> = []): IslandRegistry {
  const byName = new Map<string, IslandDefinition>();

  const registry: IslandRegistry = {
    register(definition) {
      const name = definition?.name?.trim();

      // Thrown rather than reported: this runs in the deck's setup module, not
      // on the presentation path, and an island with no name or no mount can
      // never be selected by anything. Failing at registration puts the error
      // where the mistake is instead of on slide 40.
      if (!name) {
        throw new TypeError('an island definition needs a name, e.g. { name: "vue", … }');
      }
      if (typeof definition.mount !== "function") {
        throw new TypeError(`the island "${name}" has no mount function`);
      }

      // Last registration wins. Hot module replacement re-runs a deck's setup
      // module on every edit, and throwing on the second pass would make
      // islands unusable in dev for a mistake nobody made.
      byName.set(name, definition);
    },

    lookup(name) {
      // Attributes pick up whitespace from templating and from hand-written
      // markup. `"vue "` is not a different framework.
      return byName.get(name.trim());
    },

    has(name) {
      return registry.lookup(name) !== undefined;
    },

    names() {
      return [...byName.keys()].sort();
    },
  };

  for (const definition of definitions) registry.register(definition);

  return registry;
}

/**
 * What to say when a slide asks for an island that is not there.
 *
 * Naming the registered set is the whole point. "unknown island vue" sends an
 * author looking at their frontmatter; "registered: react, three" tells them
 * in one line that the frontmatter is fine and the setup module is not.
 *
 * The case hint exists because frontmatter is prose-adjacent and `Vue` is what
 * a person writes when they are thinking about the framework rather than about
 * a token.
 */
export function unknownIslandMessage(name: string, known: readonly string[]): string {
  if (known.length === 0) {
    return (
      `unknown island "${name}" — no islands are registered; ` +
      "a deck must register one before a slide can use it"
    );
  }

  const wanted = name.trim().toLowerCase();
  const nearMiss = known.find((candidate) => candidate.toLowerCase() === wanted);
  const hint = nearMiss === undefined ? "" : `; did you mean "${nearMiss}"?`;

  return `unknown island "${name}" — registered: ${known.join(", ")}${hint}`;
}
