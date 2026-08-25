/**
 * The documentation site, on Ox Content 3.
 *
 * Authored pages live in `content/` and are the files GitHub renders. `prepare`
 * fills generated tables and rewrites links that only work in the repository,
 * then this plugin builds the HTML a reader sees.
 */

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { defineConfig } from "vite";
import { defaultTheme, defineTheme, oxContent } from "@ox-content/vite-plugin";

const here = dirname(fileURLToPath(import.meta.url));
const tokens = JSON.parse(readFileSync(join(here, "../assets/brand/tokens.json"), "utf8")) as {
  color: {
    light: { paper: string; ink: string; muted: string; signal: string; line: string };
    dark: { paper: string; ink: string; muted: string; signal: string; line: string };
  };
  typography: { fontSans: string; fontMono: string };
};

const navigationPath = join(here, ".generated/navigation.json");
let navigation: { title: string; items: { title: string; path: string }[] }[] = [];
try {
  navigation = JSON.parse(readFileSync(navigationPath, "utf8")) as typeof navigation;
} catch {
  // `docs:dev` and `docs:build` run prepare first. Opening this file alone
  // should not crash; the sidebar is empty until the generated tree exists.
}

const light = tokens.color.light;
const dark = tokens.color.dark;

export default defineConfig({
  publicDir: "public",
  plugins: [
    oxContent({
      srcDir: ".generated",
      outDir: "dist",
      highlight: true,
      headingPermalinks: true,
      docs: false,
      embeds: false,
      ssg: {
        siteName: "slidx",
        pagination: true,
        navigation,
        theme: defineTheme({
          extends: defaultTheme,
          aside: true,
          headingPermalink: "hover",
          entryPage: { mode: "subtle" },
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
