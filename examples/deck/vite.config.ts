import { defineConfig } from "vite";

import { slidx } from "@slidx/vite-plugin";

// The whole configuration. `slidx()` finds ./slides, serves them in dev, and
// emits static HTML on build.
export default defineConfig({
  plugins: [slidx()],
});
