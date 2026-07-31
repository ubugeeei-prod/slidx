import { playwright } from "@vitest/browser-playwright";
import { defineConfig } from "vite-plus";

export default defineConfig({
  optimizeDeps: {
    include: ["@vitest/browser/context", "vite-plus/test/browser/context"],
  },
  test: {
    include: ["packages/editor/test/**/*.browser.test.ts"],
    browser: {
      enabled: true,
      headless: true,
      provider: playwright(),
      instances: [{ browser: "chromium" }],
      viewport: { width: 1440, height: 900 },
    },
  },
});
