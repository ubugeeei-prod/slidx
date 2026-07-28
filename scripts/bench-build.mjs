/**
 * How long a deck takes to build, and how large it lands.
 *
 * Both numbers are claims slidx makes in its README, so both are measured
 * rather than asserted. Run it before changing anything in the pipeline that
 * you expect to be free.
 *
 * ```sh
 * node scripts/bench-build.mjs        # 100 slides
 * node scripts/bench-build.mjs 500
 * ```
 */

import { mkdtemp, mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { gzipSync } from "node:zlib";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { build } from "vite";

// The published package, not the source: this measures what a user installs,
// including whatever the library build does to it.
import { slidx } from "@slidx/vite-plugin";

const SLIDES = Number(process.argv[2] ?? 100);
const RUNS = 5;

/**
 * A slide of realistic weight.
 *
 * Benchmarking `# Hello` would measure process startup. This has a heading,
 * prose, a list that becomes steps, inline code, and speaker notes — the shape
 * of a slide someone actually writes.
 */
function slide(index) {
  if (index === 1) {
    return "---\ntitle: Benchmark\nduration: 45m\n---\n\n# Benchmark\n\nA deck of realistic slides.\n";
  }

  return `---
autoSteps: list
---

## Section ${index}

Some prose introducing the section, of the length prose usually is.

- A point of a realistic length for a slide
- Another one, with \`inline code\` and **emphasis**
- A third, mentioning [a marked phrase]{#key${index} .accent}

<!-- notes:
Something the speaker meant to say about section ${index}.
-->
`;
}

const root = await mkdtemp(join(tmpdir(), "slidx-bench-"));
await mkdir(join(root, "slides"), { recursive: true });

await Promise.all(
  Array.from({ length: SLIDES }, (_, index) =>
    writeFile(join(root, "slides", `${String(index + 1).padStart(4, "0")}.md`), slide(index + 1)),
  ),
);

process.stdout.write(`${SLIDES} slides, ${RUNS} runs\n\n`);

const timings = [];

for (let run = 0; run < RUNS; run += 1) {
  await rm(join(root, "dist"), { recursive: true, force: true });

  // Wall clock, not a profiler: it is what a person waits for.
  const started = process.hrtime.bigint();
  await build({
    root,
    logLevel: "silent",
    plugins: [slidx()],
    build: { outDir: join(root, "dist") },
  });
  const ms = Number(process.hrtime.bigint() - started) / 1e6;

  timings.push(ms);
  process.stdout.write(`  run ${run + 1}  ${ms.toFixed(0)} ms\n`);
}

timings.sort((a, b) => a - b);
const median = timings[Math.floor(timings.length / 2)];

const pages = await walk(join(root, "dist"));
const audience = pages.filter((page) => !page.includes("presenter") && page.endsWith(".html"));

let bytes = 0;
let gzipped = 0;
for (const page of audience) {
  const source = await readFile(join(root, "dist", page));
  bytes += source.byteLength;
  gzipped += gzipSync(source).byteLength;
}

const scripts = pages.filter((page) => page.endsWith(".js"));

process.stdout.write(
  `\nmedian     ${median.toFixed(0)} ms  (${(median / SLIDES).toFixed(2)} ms per slide)\n` +
    `pages      ${pages.length} files, ${audience.length} audience slides\n` +
    `size       ${(bytes / audience.length / 1024).toFixed(1)} kB per slide, ` +
    `${(gzipped / audience.length / 1024).toFixed(1)} kB gzipped\n` +
    `scripts    ${scripts.length} (${scripts.join(", ") || "none"})\n`,
);

await rm(root, { recursive: true, force: true });

async function walk(directory, prefix = "") {
  const found = [];

  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) found.push(...(await walk(join(directory, entry.name), path)));
    else found.push(path);
  }

  return found.sort((a, b) => a.localeCompare(b, "en"));
}
