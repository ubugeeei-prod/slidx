import { playwright } from "@vitest/browser-playwright";
import { defineConfig } from "vite-plus";

export default defineConfig({
  test: {
    include: ["packages/editor/test/**/*.browser.test.ts"],
    passWithNoTests: true,
    browser: {
      enabled: true,
      headless: true,
      provider: playwright(),
      instances: [{ browser: "chromium" }],
      viewport: { width: 1440, height: 900 },
    },
  },
});
