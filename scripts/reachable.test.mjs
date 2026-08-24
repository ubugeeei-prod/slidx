import { describe, expect, it } from "vite-plus/test";

import { barrelExports, importsIn, PUBLIC_API, UNREACHABLE, walk } from "./reachable.mjs";

describe("importsIn", () => {
  it("reads an import written into a page by Rust, where every brace is doubled", () => {
    // The reason this check exists. `format!` escapes a literal brace by
    // repeating it, so the statement a browser receives as `import { … }` is
    // `import {{ … }}` in the file that emits it, and a tool that parses this
    // file as Rust sees no import at all.
    const found = importsIn(`r#"import {{ createStage, LAST_STEP }} from "{runtime_src}";"#`);

    expect(found).toEqual([{ specifier: "{runtime_src}", names: ["createStage", "LAST_STEP"] }]);
  });

  it("reads an import written into a page by a template literal", () => {
    const found = importsIn('const page = `<script>import { mount } from "${EDITOR_MODULE}";');

    expect(found).toEqual([{ specifier: "${EDITOR_MODULE}", names: ["mount"] }]);
  });

  it("reads one spread over several lines, which is how the presenter page asks", () => {
    const found = importsIn(`import {
  createTimer,
  formatDuration,
} from "@slidxjs/runtime";`);

    expect(found).toEqual([
      { specifier: "@slidxjs/runtime", names: ["createTimer", "formatDuration"] },
    ]);
  });

  it("records a renamed import under the name the barrel is asked about", () => {
    expect(importsIn('import { createStage as stage } from "./stage";')).toEqual([
      { specifier: "./stage", names: ["createStage"] },
    ]);
  });

  it("leaves a type import out, because no page can ask for one at runtime", () => {
    expect(importsIn('import { type Stage, createStage } from "./stage";')).toEqual([
      { specifier: "./stage", names: ["createStage"] },
    ]);
  });
});

describe("barrelExports", () => {
  it("separates what a page could import from what erases", () => {
    const { values, types } = barrelExports(`
export { createStage, LAST_STEP } from "./stage";
export type { Stage } from "./stage";
export { type Frame, createTimer } from "./timer";
export const JS_ATTRIBUTE = "data-slidx-js";
export function markScriptEnabled() {}
`);

    expect([...values].sort()).toEqual([
      "JS_ATTRIBUTE",
      "LAST_STEP",
      "createStage",
      "createTimer",
      "markScriptEnabled",
    ]);
    expect([...types].sort()).toEqual(["Frame", "Stage"]);
  });
});

describe("walk", () => {
  const modules = (entries) =>
    new Map(entries.map(([path, source, barrel]) => [path, { source, barrel: barrel === true }]));

  const resolve = (from, specifier) =>
    specifier.startsWith(".") ? `${specifier.slice(2)}.ts` : specifier;

  it("opens only the module the requested name comes from", () => {
    // The whole check. A barrel names every module in the package, so treating
    // arrival at one as arrival at all of them would report nothing, ever.
    const { reached } = walk({
      modules: modules([
        [
          "index.ts",
          'export { createStage } from "./stage";\nexport { assessPace } from "./pace";',
          true,
        ],
        ["stage.ts", ""],
        ["pace.ts", ""],
      ]),
      entries: [{ path: "index.ts", names: ["createStage"] }],
      resolve,
    });

    expect(reached.has("stage.ts")).toBe(true);
    expect(reached.has("pace.ts")).toBe(false);
  });

  it("opens what the barrel itself imports when the name asked for is written there", () => {
    // `mount` is defined in the editor's own `index.ts` and composes twenty
    // surfaces. A rule that only followed re-exports would report all twenty.
    const { reached } = walk({
      modules: modules([
        [
          "index.ts",
          'import { createOutline } from "./outline";\nexport function mount() {}',
          true,
        ],
        ["outline.ts", ""],
      ]),
      entries: [{ path: "index.ts", names: ["mount"] }],
      resolve,
    });

    expect(reached.has("outline.ts")).toBe(true);
  });

  it("follows every import of a module that is not a barrel", () => {
    const { reached } = walk({
      modules: modules([
        ["index.ts", 'export { createStage } from "./stage";', true],
        ["stage.ts", 'import { findAnchors } from "./anchor";'],
        ["anchor.ts", ""],
      ]),
      entries: [{ path: "index.ts", names: ["createStage"] }],
      resolve,
    });

    expect(reached.has("anchor.ts")).toBe(true);
  });

  it("arrives at one barrel twice for two names, and opens both modules", () => {
    // A visited set alone would stop at the first arrival and report the second
    // module as unreached — which is a false failure rather than a missed one,
    // and therefore the failure mode worth a test.
    const { reached } = walk({
      modules: modules([
        ["index.ts", 'export { a } from "./one";\nexport { b } from "./two";', true],
        ["one.ts", ""],
        ["two.ts", ""],
      ]),
      entries: [
        { path: "index.ts", names: ["a"] },
        { path: "index.ts", names: ["b"] },
      ],
      resolve,
    });

    expect(reached.has("one.ts")).toBe(true);
    expect(reached.has("two.ts")).toBe(true);
  });

  it("terminates on a cycle", () => {
    const { reached } = walk({
      modules: modules([
        ["one.ts", 'import { b } from "./two";'],
        ["two.ts", 'import { a } from "./one";'],
      ]),
      entries: [{ path: "one.ts", names: ["a"] }],
      resolve,
    });

    expect(reached.size).toBe(2);
  });
});

describe("the recorded lists", () => {
  it("give every exempt package a reason long enough to be one", () => {
    for (const [name, reason] of Object.entries(PUBLIC_API)) {
      expect(reason.length, name).toBeGreaterThan(40);
    }
  });

  it("point every carried module at an issue rather than at nothing", () => {
    for (const [path, issue] of Object.entries(UNREACHABLE)) {
      expect(path, path).toMatch(/^packages\/[^/]+\/src\/.+\.ts$/);
      expect(Number.isInteger(issue), path).toBe(true);
    }
  });
});
