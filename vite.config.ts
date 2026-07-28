/**
 * The slidx task graph.
 *
 * Vite+ is the task runner for the whole repository — Rust and TypeScript
 * alike. Cargo and `vp` both know how to cache their own work, but neither
 * knows about the other, so the dependency edges between them live here. That
 * is what makes `vp run workspace:ci` reproduce CI exactly, rather than
 * approximating it with a list of commands that drift apart.
 *
 * Task names follow `<verb>:<subject>`. The `workspace:*` tasks are the ones a
 * person types; everything else is a node in the graph.
 */

import { spawnSync } from "node:child_process";

import { defineConfig } from "vite-plus";

/**
 * Vite+'s own `build`, `check`, `fmt`, and `test` commands bypass `run.tasks`,
 * so typing `vp check` at the root would run only the TypeScript half and
 * silently skip Rust. Delegating the bare commands into the task graph makes
 * the short form and the long form mean the same thing.
 *
 * The environment variable breaks the recursion: tasks that genuinely want the
 * built-in wrap their command in `builtin()`.
 */
const LIFECYCLE_ENV = "SLIDX_VP_LIFECYCLE";

const LIFECYCLE_TASKS: Record<string, string> = {
  build: "workspace:build",
  check: "workspace:check",
  fmt: "workspace:fmt",
  lint: "workspace:lint",
  test: "workspace:test",
};

function delegateLifecycleCommand(): void {
  if (process.env[LIFECYCLE_ENV]) return;

  const [, , command, ...args] = process.argv;
  const taskName = command ? LIFECYCLE_TASKS[command] : undefined;

  // Only the bare form delegates. `vp test some/path` is a deliberate
  // narrowing and should reach the built-in unchanged.
  if (!taskName || args.length > 0) return;

  const result = spawnSync("vp", ["run", taskName], {
    env: { ...process.env, [LIFECYCLE_ENV]: "1" },
    stdio: "inherit",
  });

  if (result.error) throw result.error;
  process.exit(result.status ?? 1);
}

delegateLifecycleCommand();

interface TaskOptions {
  cwd?: string;
  dependsOn?: string[];
  cache?: false;
}

const task = (command: string, options: TaskOptions = {}) => ({ command, ...options });

/** A task that exists only to group others. */
const group = (dependsOn: string[], options: TaskOptions = {}) =>
  task('node -e ""', { dependsOn, ...options });

/** A task whose output is not worth caching, or is not a pure function of the tree. */
const uncached = (command: string, options: Omit<TaskOptions, "cache"> = {}) =>
  task(command, { ...options, cache: false });

/** Reaches Vite+'s built-in command instead of re-entering the task graph. */
const builtin = (command: string) => `${LIFECYCLE_ENV}=1 ${command}`;

export default defineConfig({
  fmt: {
    ignorePatterns: ["**/target/**", "**/dist/**"],
  },

  lint: {
    options: {
      typeAware: true,
    },
  },

  test: {
    // The runtime package manipulates real DOM nodes: the anchor contract is
    // about where an element ends up in a tree, which cannot be asserted
    // against a mock.
    environment: "happy-dom",
    passWithNoTests: true,
  },

  run: {
    cache: {
      scripts: true,
      tasks: true,
    },

    tasks: {
      // What to type before pushing. Runs everything CI runs.
      "workspace:ci": group(["ci:conventions", "ci:rust", "ci:ts", "ci:build"]),

      // CI schedules these one per job, so a failure names the area rather
      // than the repository. They are still nodes in this graph rather than
      // steps in a workflow file — a check that exists here cannot go missing
      // from CI, because CI has no steps of its own to forget.
      "ci:conventions": group(["check:conventions", "check:version"]),
      "ci:rust": group(["fmt:rust-check", "lint:rust", "test:rust"]),
      "ci:ts": group(["fmt:ts-check", "check:ts", "test:ts"]),
      "ci:build": group(["build:rust", "build:packages"]),

      "workspace:check": group([
        "check:conventions",
        "check:version",
        "fmt:rust-check",
        "fmt:ts-check",
        "lint:rust",
        "check:ts",
      ]),
      "workspace:test": group(["test:rust", "test:ts"]),
      "workspace:build": group(["build:rust", "build:packages"]),
      "workspace:fmt": group(["fmt:rust", "fmt:ts"]),
      "workspace:lint": group(["lint:rust", "check:ts"]),

      // Rust.
      //
      // Everything that compiles is `uncached`. Cargo already has a good
      // incremental cache and it lives in `target/`, inside the workspace, so
      // Vite+ sees each of these tasks read and write its own inputs and
      // declines to cache them anyway. Saying so explicitly is honest — it
      // keeps the run summary free of "not cached" noise that looks like a
      // problem, and it makes clear that cargo owns Rust incrementality.
      //
      // `fmt:rust` is the exception: it touches nothing under `target/`, so it
      // caches like any other task.
      //
      // `check:rust` runs before clippy so a type error is reported as a type
      // error rather than buried in lint output.
      "check:rust": uncached("cargo check --workspace --all-targets"),
      "lint:rust": uncached("cargo clippy --workspace --all-targets -- -D warnings", {
        dependsOn: ["check:rust"],
      }),
      "fmt:rust": task("cargo fmt --all"),
      "fmt:rust-check": task("cargo fmt --all -- --check"),
      "test:rust": uncached("cargo test --workspace"),
      "test:rust-verbose": uncached("cargo test --workspace -- --nocapture"),
      "build:rust": uncached("cargo build --workspace --release"),
      "doc:rust": uncached("cargo doc --workspace --no-deps"),

      // The wasm package is the boundary every JavaScript consumer goes
      // through, so nothing on that side can be checked until it exists.
      "build:wasm": uncached("node scripts/build-wasm.mjs"),

      // The runtime is consumed through its published `exports`, which point
      // at `dist/`. Importing it from source instead would test a module that
      // no user ever loads, so it is built before anything reads it. This is
      // the edge CI caught and a local tree hides: `dist/` is gitignored and
      // is already there on a machine that has run a build once.
      "build:runtime": uncached("vp run --filter @slidx/runtime pack:lib"),

      "build:packages": group(["build:wasm", "build:runtime"]),

      // TypeScript.
      "check:ts": task(builtin("vp check"), { dependsOn: ["build:packages"] }),
      "fmt:ts": task(builtin("vp fmt")),
      "fmt:ts-check": task(builtin("vp fmt --check")),

      // Uncached for a mechanical reason rather than a principled one: loading
      // this TypeScript config writes a transpiled copy into
      // `node_modules/.vite-temp`, which reads as the task modifying its own
      // input. Marking it explicitly keeps the run summary honest instead of
      // reporting a miss on every run.
      "test:ts": uncached(builtin("vp test"), { dependsOn: ["build:packages"] }),

      "check:conventions": task("node scripts/check-conventions.mjs"),
      "check:version": task("node scripts/check-version.mjs"),

      // The README images are output of the pipeline, not artwork. Kept as a
      // task so regenerating them is one command and never a manual crop.
      "preview:deck": uncached("vp exec --filter slidx-example-deck -- vite build"),
      screenshots: uncached("node scripts/screenshot.mjs", { dependsOn: ["preview:deck"] }),

      // Benchmarks measure wall-clock time, so a cached result is a wrong one.
      "bench:rust": uncached("cargo bench --workspace"),
    },
  },
});
