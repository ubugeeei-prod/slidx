/**
 * Which modules no page can run.
 *
 * `check-dead-config.mjs` catches the shape that produced
 * `ShellOptions::include_runtime` — a `pub` field nothing reads. This is the
 * other half, and it was written after five M4 features sat written, tested and
 * reachable by nobody at the same time: presentation mode, the behind/ahead
 * reading, the demo switch, the phone remote, and the whole audience channel.
 *
 * # Why an off-the-shelf tool cannot do this
 *
 * A runtime symbol's real call site here is **a string literal in another
 * language**. `slidx_render` emits
 *
 * ```text
 * import { createStage, createNavigator } from "{runtime_src}";
 * ```
 *
 * into the page it renders, and the Vite plugin emits the editor's
 * `import { mount }` the same way. A dead-export tool parses those files as
 * Rust and as TypeScript, finds no import in either, and concludes that the
 * whole runtime is unused and every editor module is used — exactly backwards
 * on both counts. So the walk reads import statements wherever they appear,
 * including inside a string.
 *
 * # What reachable means
 *
 * A path from something a person can open, walked one file at a time. The roots
 * are declarations rather than deductions, because what reaches *them* is
 * outside this workspace: a browser opens the page the crates render, and a
 * deck's own `vite.config.ts` imports the plugin.
 *
 * A **barrel is not a door**. Arriving at `index.ts` does not reach everything
 * it re-exports — `export { assessPace } from "./pace"` opens `pace.ts` only for
 * somebody who asked for `assessPace`. That distinction is the whole check:
 * every unreachable feature in M4 is a module its barrel names and nothing
 * requests.
 *
 * This is a claim about use rather than about evaluation. A real ESM barrel does
 * evaluate every module it re-exports; the question here is whether any path
 * from a page can *call* what is in one, and a name nobody imports has none.
 *
 * Names defined *in* a barrel are the exception, and a real one rather than a
 * convenience. `mount` is written in the editor's `index.ts` and composes twenty
 * surfaces, so asking for it does reach every module `index.ts` imports. A rule
 * that only followed re-exports would report the entire editor.
 *
 * # What it can and cannot claim
 *
 * **An unreached module is conclusive**, and is the only thing reported. No
 * import of it exists on any path from a page, so nothing in it can be called,
 * whatever its tests say.
 *
 * **An unimported export is not reported**, though the walk records enough to.
 * A barrel offering a name nobody takes is true of a feature nobody wired up and
 * equally true of `HIDDEN_ATTRIBUTE` — a constant naming an attribute the Rust
 * half writes, exported so a test can hold the two halves together. Thirty of
 * those would sit above the finding that matters, and a check whose output has
 * to be triaged is a check people stop reading. Reporting them needs an
 * exemption list of its own, which is work rather than a flag.
 *
 * **Types are not reported at all.** A type cannot appear in a page's
 * `import { … }` at runtime, and `export type` erases, so a module that exports
 * only types ships nothing and runs nothing. Treating one as unreached would
 * report every shape in the workspace and be ignored inside a week.
 */

/**
 * Which emitted bundle an import specifier names.
 *
 * These are the specifiers that appear inside a string literal, where the text
 * is a placeholder rather than a package: `slidx_render` interpolates the
 * runtime's own emitted filename, and the editor page interpolates a route.
 * Nothing can resolve them by reading the string, so the mapping is recorded
 * here with the reason.
 *
 * A rename is loud rather than silent. If `runtime_src` becomes something else,
 * every runtime module stops looking reached at once and the check reports the
 * whole package — a failure nobody can miss, unlike the reverse.
 */
export const EMITTED_BUNDLES = {
  "{runtime_src}": "@slidxjs/runtime/emitted",
  "{camera_src}": "@slidxjs/runtime/camera",
  "{presenter_runtime_src}": "@slidxjs/runtime/presenter",
  "{rehearsal_src}": "@slidxjs/rehearsal",
  "${EDITOR_MODULE}": "@slidxjs/editor",
};

/**
 * Packages whose consumer is somebody else's code, and why.
 *
 * A reason is required, for the purpose it serves in `write-only.mjs`: a bare
 * list of names is how an exemption outlives the reason it was granted for.
 *
 * Every entry is a package that ships *to be imported by a deck*, so the absence
 * of an importer in this workspace is the design rather than the defect.
 */
export const PUBLIC_API = {
  "@slidxjs/islands":
    "An opt-in integration a deck author imports in their own component. Nothing here should import it — a slidx package depending on Vue or React is the lock-in the non-goals refuse.",
  "@slidxjs/publish":
    "The payload builders behind `slidx publish`, exported so a deck's own CI can compose one without shelling out to the binary.",
  "@slidxjs/theme-workshop":
    "A theme author's tooling. It reads a token document and reports on it; no shipped page has a reason to.",
  "@slidxjs/wasm":
    "Generated bindings. `wasm-bindgen` writes the barrel, so an export nobody imports is a Rust `#[wasm_bindgen]` item, and `check-dead-config.mjs` is the check that can see one.",
  slidx: "The binary's npm wrapper. Its consumer is a shell, not an import.",
  "slidx-vscode": "An extension host loads it. There is no importer to have.",
};

/**
 * What is unreachable today, and which issue closes it.
 *
 * A ratchet rather than an exemption list, and the difference is in both
 * directions. A path here does not fail the check; a path *not* here does, so
 * nothing new can join the list quietly. And a path here that has *become*
 * reachable fails too, so the list shrinks by itself as the work lands rather
 * than outliving it.
 *
 * That second half is why this is not `PUBLIC_API`. An exemption says the
 * absence of an importer is correct. Every line below says the opposite: it is
 * a feature that is written, tested, and shipped to nobody, and the number
 * beside it is where that gets fixed.
 *
 * Two of the seventeen it opened with were not known before it ran, and
 * finding them is the argument for having built it: an editor surface with no
 * constructor call anywhere in the workspace — not even in a test — and the
 * rehearsal comparison across runs, in a package whose single-run report did
 * reach the presenter view. Both are gone.
 */
export const UNREACHABLE = {
  "packages/audience/src/backoff.ts": 281,
  "packages/audience/src/client.ts": 281,
  "packages/audience/src/index.ts": 281,
  "packages/audience/src/participant.ts": 281,
  "packages/audience/src/protocol.ts": 281,
  "packages/audience/src/questions.ts": 281,
  "packages/audience/src/room.ts": 281,
  "packages/audience/src/routes.ts": 281,
  "packages/audience/src/worker.ts": 281,
  "packages/runtime/src/demo.ts": 279,
  "packages/runtime/src/media.ts": 286,
};

/**
 * A bundle a page downloads whole, and the names it may therefore carry.
 *
 * `readRuntime()` reads the packed entry as a file and the plugin emits it
 * without ever putting it through the deck's own bundler, so nothing shakes an
 * export nobody imports out of it. That made the runtime 47% larger than the
 * eleven names a page asks for.
 *
 * The rule is equality rather than "at least": a name a page imports and the
 * bundle does not export is a deck that breaks on load, and a name the bundle
 * exports and no page imports is the 47% again. Both directions are reported.
 *
 * Both halves of the runtime are listed, for different reasons. The first is
 * what a room downloads and the rule holds its size down. The second is on the
 * speaker's own screen, where a few kilobytes do not matter — but it is where
 * a presenter-side feature *belongs*, and a name that drifted back into the
 * first is one an audience pays for. Reporting the extras on both makes that
 * drift visible in whichever direction it happens.
 *
 * The editor's bundle is not listed: it is served from `configureServer`, so
 * it reaches nobody but its author, and its barrel is a composition root
 * rather than a list of names a page asks for.
 */
export const EMITTED_EXACTLY = {
  "@slidxjs/runtime/emitted": "{runtime_src}",
  "@slidxjs/runtime/presenter": "{presenter_runtime_src}",
};

/**
 * Bundles that are a module rather than a list, so the equality rule cannot
 * apply.
 *
 * `@slidxjs/runtime/camera` is `camera.ts` itself: it is emitted whole for a
 * deck that places a camera, and its exports are what that module is, not a
 * list somebody keeps in step with a page. Holding it to the two names a slide
 * imports would mean deleting the types beside them, which erase anyway.
 *
 * Recorded rather than left out, so that "why is this one not checked" has an
 * answer where the check is.
 */
export const EMITTED_WHOLE = ["@slidxjs/runtime/camera"];

/** Barrel exports, split by what a page could actually import. */
export function barrelExports(source) {
  const values = new Set();
  const types = new Set();

  for (const match of source.matchAll(/export\s+type\s*\{([^}]*)\}/g)) {
    for (const name of clause(match[1]).values) types.add(name);
  }

  for (const match of source.matchAll(/export\s*\{([^}]*)\}/g)) {
    const { values: named, types: inline } = clause(match[1]);
    for (const name of inline) types.add(name);
    for (const name of named) values.add(name);
  }

  for (const match of source.matchAll(/export\s+(?:const|function|class)\s+(\w+)/g)) {
    values.add(match[1]);
  }

  for (const name of types) values.delete(name);

  return { values, types };
}

/**
 * Where a barrel's names come from.
 *
 * `from` maps a re-exported name to the specifier behind it, which is what lets
 * a request for one name open one module. `local` is the names the barrel
 * defines itself, and asking for one of those reaches whatever the barrel
 * imports — a function written there can use nothing else.
 */
export function barrelBindings(source) {
  const from = new Map();
  const local = new Set();

  for (const match of source.matchAll(/export\s*\{([^}]*)\}\s*from\s*["']([^"']+)["']/g)) {
    for (const name of clause(match[1]).values) from.set(name, match[2]);
  }

  for (const match of source.matchAll(/export\s+(?:const|function|class)\s+(\w+)/g)) {
    local.add(match[1]);
  }

  return { from, local };
}

/**
 * Every `import { … } from "…"` in a source, wherever it appears.
 *
 * Deliberately blind to whether it is code or a string. The whole point is the
 * ones that are strings, and a rule that told them apart would have to
 * understand two languages to reach the same answer.
 *
 * The doubled braces are Rust's, not a typo. `slidx_render` writes these imports
 * through `format!`, where a literal brace is escaped by repeating it, so a
 * statement that reaches a browser as `import { createStage }` is
 * `import {{ createStage }}` in the file that emits it.
 */
export function importsIn(source) {
  return [...source.matchAll(/import\s*\{\{?([^{}]*)\}?\}\s*from\s*["'`]([^"'`]+)["'`]/g)].map(
    (match) => ({ specifier: match[2], names: clause(match[1]).values }),
  );
}

/**
 * Splits an import or export clause into names.
 *
 * `as` is dropped because a page importing `createStage as stage` still reads
 * the export by the name the barrel is asked about.
 */
function clause(text) {
  const values = [];
  const types = [];

  for (const part of text.split(",")) {
    const trimmed = part.trim();
    if (trimmed === "") continue;

    const inline = /^type\s+/.test(trimmed);
    const name = trimmed
      .replace(/^type\s+/, "")
      .split(/\s+as\s+/)[0]
      .trim();
    if (!/^\w+$/.test(name)) continue;

    (inline ? types : values).push(name);
  }

  return { values, types };
}

/**
 * Walks the graph from the roots and reports where it never arrives.
 *
 * `modules` maps a path to `{ source, barrel }`. `resolve(from, specifier)`
 * answers with a path in `modules` or `undefined`, and is injected so the rules
 * can be exercised without a filesystem. `entries` is what the roots ask for:
 * `{ path, names }`, one per import statement a root emits.
 *
 * A module is arrived at once per set of names, not once: a barrel reached for
 * `createStage` and later for `createTimer` has to open a different module each
 * time, so stopping at the first arrival would report the second as unreached.
 */
export function walk({ modules, entries, resolve }) {
  const reached = new Set();
  /** Names asked of each module by something already reached. */
  const requested = new Map();
  /** Names already forwarded from each module, so the walk terminates. */
  const forwarded = new Map();

  const frontier = entries.map((entry) => ({ path: entry.path, names: entry.names }));

  while (frontier.length > 0) {
    const { path, names } = frontier.pop();
    const module = modules.get(path);
    if (module === undefined) continue;

    const asked = requested.get(path) ?? new Set();
    for (const name of names) asked.add(name);
    requested.set(path, asked);

    const before = forwarded.get(path);
    const fresh = before === undefined || names.some((name) => !before.has(name));
    if (!fresh) continue;

    reached.add(path);
    const done = before ?? new Set();
    for (const name of names) done.add(name);
    forwarded.set(path, done);

    const bindings = module.barrel ? barrelBindings(module.source) : undefined;

    // A barrel forwards what it was asked for. Anything else it imports belongs
    // to a name nobody requested, and following it would reach every module in
    // the package from a single import of one.
    if (bindings === undefined || names.some((name) => bindings.local.has(name))) {
      for (const { specifier, names: imported } of importsIn(module.source)) {
        const next = resolve(path, specifier);
        if (next !== undefined) frontier.push({ path: next, names: imported });
      }
    }

    for (const name of bindings === undefined ? [] : names) {
      const specifier = bindings.from.get(name);
      if (specifier === undefined) continue;

      const next = resolve(path, specifier);
      if (next !== undefined) frontier.push({ path: next, names: [name] });
    }
  }

  return { reached, requested };
}
