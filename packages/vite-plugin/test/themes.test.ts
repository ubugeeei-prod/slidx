/**
 * A theme installed from npm, from the manifest on disk to the built page.
 *
 * The acceptance line on the issue is that an installed theme package renders
 * *without any core change*, and the only way to check that is to install one:
 * a directory in `node_modules`, a `package.json` naming its document, and a
 * deck whose frontmatter says the theme's id and nothing else. Every layer this
 * crosses — dependency discovery here, hardening and resolution in Rust, the
 * stylesheet in the emitted page — is exercised by the fact that the page comes
 * out in the theme's own colours.
 *
 * The document is generated from `slidx_theme::published`, so these fixtures
 * describe the same theme `@slidx/theme-workshop` publishes rather than a
 * plausible copy of one.
 */

import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { build } from "vite";
import { describe, expect, it } from "vite-plus/test";

import { slidx } from "../src/index";
import { readThemePackages } from "../src/themes";

const packages = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

/** The document `@slidx/theme-workshop` ships, read from the package itself. */
async function workshopDocument(): Promise<string> {
  return readFile(join(packages, "theme-workshop", "theme.json"), "utf8");
}

interface Installed {
  /** What goes in the dependent's `package.json`. */
  dependency?: boolean;
  manifest?: Record<string, unknown>;
  files?: Record<string, string>;
}

/** A project with a package installed into its own `node_modules`. */
async function project(name: string, installed: Installed): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "slidx-theme-"));
  const directory = join(root, "node_modules", ...name.split("/"));

  await mkdir(directory, { recursive: true });
  await writeFile(
    join(root, "package.json"),
    JSON.stringify({
      name: "deck",
      ...(installed.dependency === false ? {} : { devDependencies: { [name]: "*" } }),
    }),
  );

  if (installed.manifest !== undefined) {
    await writeFile(
      join(directory, "package.json"),
      JSON.stringify({ name, version: "0.0.0", ...installed.manifest }),
    );
  }

  for (const [file, contents] of Object.entries(installed.files ?? {})) {
    await writeFile(join(directory, file), contents);
  }

  return root;
}

/** A project with `@slidx/theme-workshop` installed the way npm would. */
async function withWorkshop(): Promise<string> {
  return project("@slidx/theme-workshop", {
    manifest: { slidx: { theme: "./theme.json" } },
    files: { "theme.json": await workshopDocument() },
  });
}

describe("finding the theme packages a project installed", () => {
  it("reads the document a dependency names in its own manifest", async () => {
    const found = await readThemePackages(await withWorkshop());

    expect(found).toHaveLength(1);
    expect(found[0]?.source).toBe("@slidx/theme-workshop");
    expect(JSON.parse(found[0]?.document ?? "{}")).toMatchObject({ id: "workshop" });
  });

  it("ignores a dependency that is not a theme", async () => {
    // Every project has dozens. Reading each one's manifest is the whole cost,
    // and a package with no `slidx.theme` key is simply not one of these.
    const root = await project("some-library", { manifest: { main: "./index.js" } });

    expect(await readThemePackages(root)).toEqual([]);
  });

  it("ignores a theme package the project did not ask for", async () => {
    // Installing is the declaration. A theme arriving through somebody else's
    // dependency tree would be a package the author never chose deciding what
    // their deck looks like.
    const root = await project("@evil/theme-lurker", {
      dependency: false,
      manifest: { slidx: { theme: "./theme.json" } },
      files: { "theme.json": await workshopDocument() },
    });

    expect(await readThemePackages(root)).toEqual([]);
  });

  it("refuses to read a document outside the package that names it", async () => {
    // Everything here is the author's own disk, but a dependency that can
    // point `slidx.theme` at a file elsewhere on it is a dependency that can
    // have that file's contents read into a page.
    const root = await project("@evil/theme-escape", {
      manifest: { slidx: { theme: "../../../secret.json" } },
      files: {},
    });
    await writeFile(join(root, "secret.json"), '{"id":"secret"}');

    expect(await readThemePackages(root)).toEqual([]);
  });

  it("says nothing about a project that has installed nothing at all", async () => {
    // A directory mid-install, a fresh clone, a deck with no package.json —
    // none of them is a mistake anyone could act on.
    const root = await mkdtemp(join(tmpdir(), "slidx-theme-"));

    expect(await readThemePackages(root)).toEqual([]);
  });

  it("hands the documents over in the same order every time", async () => {
    // Which is what lets the Rust side settle two packages claiming one id the
    // same way on every machine rather than by directory listing.
    const root = await withWorkshop();

    expect(await readThemePackages(root)).toEqual(await readThemePackages(root));
  });
});

describe("a deck that names an installed theme", () => {
  async function buildWith(source: string): Promise<string> {
    const root = await withWorkshop();
    await mkdir(join(root, "slides"), { recursive: true });
    await writeFile(join(root, "slides", "0001.md"), source);

    await build({
      root,
      logLevel: "silent",
      plugins: [slidx()],
      build: { outDir: join(root, "dist") },
    });

    return readFile(join(root, "dist", "slides", "index.html"), "utf8");
  }

  it("renders in the package's own colours", async () => {
    // The end of the path the issue asks for: nothing imported, nothing
    // registered, one line of frontmatter.
    const document = JSON.parse(await workshopDocument()) as {
      light: { accent: { r: number; g: number; b: number } };
    };
    const { r, g, b } = document.light.accent;
    const accent = `#${[r, g, b].map((part) => part.toString(16).padStart(2, "0")).join("")}`;

    const page = await buildWith("---\ntitle: Demo\ntheme: workshop\n---\n\n# One\n");

    expect(page).toContain(`--slidx-color-accent: ${accent}`);
  }, 60_000);

  it("still makes no request for anything", async () => {
    // The guarantee a theme package is the easiest way to break, because a
    // font stack in a published file is a long way from the rule that catches
    // a remote asset in a deck.
    const page = await buildWith("---\ntitle: Demo\ntheme: workshop\n---\n\n# One\n");

    expect(page).not.toContain("http://");
    expect(page).not.toContain("https://");
    expect(page).not.toContain("url(");
  }, 60_000);
});
