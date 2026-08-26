/**
 * The documentation site, on Ox Content 3.
 *
 * Authored pages live in `content/` and are the files GitHub renders. `prepare`
 * fills generated tables and rewrites links that only work in the repository,
 * then this plugin builds the HTML a reader sees.
 */

import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, isAbsolute, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

import { defaultTheme, defineTheme, oxContent } from "@ox-content/vite-plugin";
import { defineConfig, type Plugin } from "vite";

const here = dirname(fileURLToPath(import.meta.url));
const tokens = JSON.parse(readFileSync(join(here, "../assets/brand/tokens.json"), "utf8")) as {
  color: {
    light: { paper: string; ink: string; muted: string; signal: string; line: string };
    dark: { paper: string; ink: string; muted: string; signal: string; line: string };
  };
  typography: { fontSans: string; fontMono: string };
};

type NavGroup = { title: string; items: { title: string; path: string }[] };

const navigationPath = join(here, ".generated/navigation.json");
const navigationJaPath = join(here, ".generated/ja/navigation.json");
let navigation: NavGroup[] = [];
let navigationJa: NavGroup[] = [];
try {
  navigation = JSON.parse(readFileSync(navigationPath, "utf8")) as NavGroup[];
  navigationJa = JSON.parse(readFileSync(navigationJaPath, "utf8")) as NavGroup[];
} catch {
  // `docs:dev` and `docs:build` run prepare first. Opening this file alone
  // should not crash; the sidebar is empty until the generated tree exists.
}

/**
 * One sidebar, two labels. Ox Content rewrites the English paths onto
 * `/ja/…` when that sibling exists. A second `navigation` object named
 * `localeNavigation` is not a field it has, so the Japanese titles have
 * to travel as locale maps on this tree.
 */
function sidebarFromLocales(english: NavGroup[], japanese: NavGroup[]) {
  return english.map((group, index) => {
    const other = japanese[index];
    return {
      text: { en: group.title, ja: other?.title ?? group.title },
      items: group.items.map((item, itemIndex) => {
        const translated = other?.items[itemIndex];
        return {
          text: { en: item.title, ja: translated?.title ?? item.title },
          link: item.path,
        };
      }),
    };
  });
}

const light = tokens.color.light;
const dark = tokens.color.dark;

/**
 * Re-runs prepare when an authored page or a picture changes.
 *
 * `docs:prepare` finishes before Vite starts, and Ox Content reads
 * `.generated`, not `content/`. Without this, a save in the Markdown the
 * author actually edits would not reach the page they are looking at until
 * they restarted the server.
 *
 * `apply: "serve"` so `docs:build` does not watch anything. The production
 * task already runs prepare once, and a watcher during a build would be a
 * second copy of a tree that is not supposed to change.
 */
function prepareOnSave(): Plugin {
  const content = join(here, "content");
  const media = join(here, "media");
  const workspace = join(here, "..");

  return {
    name: "slidx-docs-prepare",
    apply: "serve",
    configureServer(server) {
      server.watcher.add(content);
      server.watcher.add(media);

      let running = false;
      let again = false;
      let timer: ReturnType<typeof setTimeout> | undefined;
      let child: ReturnType<typeof spawn> | undefined;

      const run = () => {
        if (running) {
          again = true;
          return;
        }
        running = true;
        child = spawn("cargo", ["run", "-p", "slidx_docs", "--example", "prepare"], {
          cwd: workspace,
          stdio: "inherit",
        });
        child.on("error", (error) => {
          running = false;
          child = undefined;
          server.config.logger.error(error.message);
        });
        child.on("close", () => {
          running = false;
          child = undefined;
          if (again) {
            again = false;
            run();
          }
        });
      };

      const schedule = (file: string) => {
        if (!inside(file, content) && !inside(file, media)) return;
        // An editor save is often two events — unlink of the old bytes, add
        // of the new — and cargo would otherwise run twice for one keystroke.
        if (timer !== undefined) clearTimeout(timer);
        timer = setTimeout(() => {
          timer = undefined;
          run();
        }, 50);
      };

      server.watcher.on("add", schedule);
      server.watcher.on("change", schedule);
      server.watcher.on("unlink", schedule);
      server.httpServer?.on("close", () => {
        if (timer !== undefined) clearTimeout(timer);
        child?.kill();
      });
    },
  };
}

function inside(file: string, directory: string): boolean {
  const fromDirectory = relative(directory, file);
  return fromDirectory !== "" && !fromDirectory.startsWith("..") && !isAbsolute(fromDirectory);
}

export default defineConfig({
  publicDir: "public",
  plugins: [
    prepareOnSave(),
    oxContent({
      srcDir: ".generated",
      outDir: "dist",
      highlight: true,
      headingPermalinks: true,
      docs: false,
      embeds: false,
      // Pages are translated Markdown, not ICU dictionaries. `check` looks
      // for the latter and would fail a tree that has none.
      i18n: {
        enabled: true,
        defaultLocale: "en",
        hideDefaultLocale: true,
        check: false,
        locales: [
          { code: "en", name: "English" },
          { code: "ja", name: "日本語" },
        ],
      },
      ssg: {
        siteName: "slidx",
        pagination: true,
        localeSwitcher: true,
        theme: defineTheme({
          extends: defaultTheme,
          aside: true,
          headingPermalink: "hover",
          entryPage: { mode: "subtle" },
          sidebar: sidebarFromLocales(navigation, navigationJa),
          colors: {
            primary: light.signal,
            primaryHover: light.signal,
            background: light.paper,
            backgroundAlt: light.paper,
            text: light.ink,
            textMuted: light.muted,
            border: light.line,
          },
          darkColors: {
            primary: dark.signal,
            primaryHover: dark.signal,
            background: dark.paper,
            backgroundAlt: dark.paper,
            text: dark.ink,
            textMuted: dark.muted,
            border: dark.line,
          },
          fonts: {
            sans: tokens.typography.fontSans,
            mono: tokens.typography.fontMono,
          },
          socialLinks: {
            github: "https://github.com/ubugeeei-prod/slidx",
          },
          footer: {
            message: "slidx is pre-alpha and unreleased.",
            copyright: "MIT. The repository is the whole of it.",
          },
          tokens: { radius: "0px" },
          darkTokens: { radius: "0px" },
        }),
      },
    }),
  ],
});
