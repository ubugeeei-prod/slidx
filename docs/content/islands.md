---
title: Framework islands
summary: Opt one deck into Vue, React, Svelte, Solid or Angular components without making the deck run on one.
section: reference
order: 5
---

# Framework islands

A slide is Markdown and complete HTML first. When one part genuinely needs a
component, mark that element as an island and register exactly the component
runtime it uses. The rest of the deck stays static: no client entry is emitted
without an island setup, and a page with no marked element does not load that
entry even when another slide does.

## 1. Point the deck at its setup

The setup module is relative to the Vite root:

```ts
import { defineConfig } from "vite";
import { slidx } from "@slidx/vite-plugin";

export default defineConfig({
  plugins: [
    slidx({
      islands: "./islands.ts",
    }),
  ],
});
```

That option is the deck-level opt-in. Leaving it out preserves the ordinary
zero-JavaScript audience slide.

## 2. Register only what the deck uses

This example chooses Vue. The component and Vue itself are loaded when the
island becomes visible, not when slide one opens:

```ts
import { createRegistry } from "@slidx/islands";
import { vueIsland } from "@slidx/islands/vue";

export default createRegistry([
  vueIsland({
    name: "Counter",
    component: () => import("./components/Counter.vue"),
  }),
]);
```

The framework is a per-component adapter rather than a mode the whole deck
enters. A deck can register a React chart beside that Vue counter, and removing
either registration removes its adapter and component from the Vite graph.

| Choice  | Adapter entry            | Factory         | Install in the deck                        |
| ------- | ------------------------ | --------------- | ------------------------------------------ |
| Vue     | `@slidx/islands/vue`     | `vueIsland`     | `vue` and the Vue Vite plugin              |
| React   | `@slidx/islands/react`   | `reactIsland`   | `react`, `react-dom`, and the React plugin |
| Svelte  | `@slidx/islands/svelte`  | `svelteIsland`  | `svelte` and the Svelte Vite plugin        |
| Solid   | `@slidx/islands/solid`   | `solidIsland`   | `solid-js` and the Solid Vite plugin       |
| Angular | `@slidx/islands/angular` | `angularIsland` | Angular 20+ and its compiler plugin        |

Angular components and Angular's published packages need Angular's own compiler
and linker in the deck's Vite config. The adapter runs zoneless so one island
does not patch timers, promises and event listeners for every slide.

## 3. Put a complete fallback in Markdown

The island name selects the registry entry and props cross one JSON attribute:

```md
## Sign-ups

<div
  data-slidx-island="Counter"
  data-slidx-island-props='{"start": 128, "label": "people"}'
>
  <strong>128 people</strong>
</div>
```

The children are not loading chrome. They are the static answer: social cards,
print/PDF, a failed component import, and a slide opened with JavaScript
disabled all show it. The hydrator restores the same markup if mounting fails,
so one component cannot blank the slide on stage.

Props are JSON, not executable expressions. Malformed JSON is reported and the
component mounts with an empty object, leaving the fallback available instead
of taking the rest of the page down.

## Lifecycle

An island is resolved while the page loads, mounted only when visible, and
unmounted when it leaves. Each adapter releases the framework object it owns:
the Vue app, React root, Svelte instance, Solid owner, or Angular application
and component. Returning to the slide creates one fresh mount rather than
stacking another component on the first.

The presenter view keeps its next-slide preview static. It does not start a
component one slide early, and the print document never imports the client.
