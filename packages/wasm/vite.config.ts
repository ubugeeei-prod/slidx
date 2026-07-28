import { defineConfig } from "vite-plus";

export default defineConfig({
  test: {
    // The wasm module is a Node consumer here, not a DOM one: these tests
    // check the boundary, and `happy-dom` would only slow them down.
    environment: "node",
  },
});
