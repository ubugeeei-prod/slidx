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
import { slidx } from "@ubugeeei/slidx-vite-plugin";

export default defineConfig({
  plugins: [
    slidx({
      islands: "./islands.ts",
      mdx: true,
    }),
  ],
});
```

`islands` is the client-runtime opt-in. `mdx` is separate: it adds `.mdx` to the
default slide extensions and enables component syntax. Leave both out and the
ordinary `.md` path remains unchanged, with a zero-JavaScript audience slide.

## 2. Register only what the deck uses

This example chooses Vue. The component and Vue itself are loaded when the
island becomes visible, not when slide one opens:

```ts
import { createRegistry } from "@ubugeeei/slidx-islands";
import { vueIsland } from "@ubugeeei/slidx-islands/vue";

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

| Choice  | Adapter entry                     | Factory         | Install in the deck                        |
| ------- | --------------------------------- | --------------- | ------------------------------------------ |
| Vue     | `@ubugeeei/slidx-islands/vue`     | `vueIsland`     | `vue` and the Vue Vite plugin              |
| React   | `@ubugeeei/slidx-islands/react`   | `reactIsland`   | `react`, `react-dom`, and the React plugin |
| Svelte  | `@ubugeeei/slidx-islands/svelte`  | `svelteIsland`  | `svelte` and the Svelte Vite plugin        |
| Solid   | `@ubugeeei/slidx-islands/solid`   | `solidIsland`   | `solid-js` and the Solid Vite plugin       |
| Angular | `@ubugeeei/slidx-islands/angular` | `angularIsland` | Angular 20+ and its compiler plugin        |

Angular components and Angular's published packages need Angular's own compiler
and linker in the deck's Vite config. The adapter runs zoneless so one island
does not patch timers, promises and event listeners for every slide.

## 3. Put a complete fallback in Markdown

With `mdx: true`, a capitalised tag selects the registry entry with the same
name:

```mdx
## Sign-ups

<Counter start={128} label="people">

**128 people**

</Counter>
```

String attributes and JSON values in braces become props. Arrays and objects
are allowed too. Imports are unnecessary: the setup registry resolves
`Counter`, so removing that one registration also removes its framework and
component from the Vite graph.

The compiler never executes an expression from a deck. A value such as
`start={window.total}` is a blocking `mdx/non-static-props` diagnostic, renders
the fallback without an island marker, and cannot run during the build.

The `.mdx` file remains the editor's source of truth. Visual text, style,
layout, animation, slide order, undo, and shared edits splice that file; MDX is
compiled only for rendering. Code fences and lowercase HTML are left alone.

The explicit form works in ordinary `.md` as well. The island name selects the
registry entry and props cross one JSON attribute:

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
