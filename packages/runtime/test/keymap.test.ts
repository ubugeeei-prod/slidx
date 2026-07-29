/**
 * The keyboard.
 *
 * A speaker drives a deck with their hands off the screen — from a clicker,
 * from the keyboard, in the dark. So the keymap is a *table*, not a switch
 * statement buried in a handler: it can be listed on screen, rebound, and
 * checked for the mistakes that matter.
 *
 * The mistakes that matter, and which these tests hold:
 *
 * - **Nothing may shadow the browser.** A deck that eats ⌘R or ⌘← is a deck
 *   the browser no longer works in.
 * - **Nothing may fire while someone is typing.** The editor and the audience
 *   channel both put inputs on the page, and a space bar that advances the
 *   slide mid-question is reported as the deck being haunted.
 * - **No two commands may claim the same key.** A conflict is silent: one
 *   command simply never runs, and nobody finds out until the stage.
 * - **Every command must be discoverable.** A shortcut nobody can list is a
 *   shortcut nobody uses.
 */

import { describe, expect, it } from "vite-plus/test";

import {
  createKeymap,
  DEFAULT_BINDINGS,
  formatBinding,
  type Command,
  type Keymap,
} from "../src/keymap";

function keymapWith(overrides: Partial<Record<Command, () => void>> = {}): {
  keymap: Keymap;
  ran: Command[];
} {
  const ran: Command[] = [];
  const commands = Object.fromEntries(
    DEFAULT_BINDINGS.map((binding) => [
      binding.command,
      overrides[binding.command] ??
        (() => {
          ran.push(binding.command);
        }),
    ]),
  ) as Record<Command, () => void>;

  return { keymap: createKeymap({ commands }), ran };
}

function press(keymap: Keymap, key: string, init: KeyboardEventInit = {}) {
  const event = new KeyboardEvent("keydown", { key, cancelable: true, bubbles: true, ...init });
  document.body.dispatchEvent(event);
  keymap.handle(event);
  return event;
}

describe("driving the deck", () => {
  it("advances and goes back on the keys a clicker sends", () => {
    // Presentation remotes send PageDown and ArrowRight; they do not send `n`.
    for (const key of ["ArrowRight", "PageDown", " "]) {
      const { keymap, ran } = keymapWith();
      press(keymap, key);
      expect(ran, key).toEqual(["next"]);
    }

    for (const key of ["ArrowLeft", "PageUp"]) {
      const { keymap, ran } = keymapWith();
      press(keymap, key);
      expect(ran, key).toEqual(["previous"]);
    }
  });

  it("blacks out the screen", () => {
    // The oldest presenter shortcut there is, and the one that matters most:
    // it takes the audience's eyes off the slide and puts them on you.
    const { keymap, ran } = keymapWith();
    press(keymap, "b");

    expect(ran).toEqual(["blackout"]);
  });

  it("starts and resets the timer without reaching for the mouse", () => {
    const { keymap, ran } = keymapWith();
    press(keymap, "t");
    press(keymap, "r");

    expect(ran).toEqual(["toggleTimer", "resetTimer"]);
  });

  it("opens the presenter view, the overview, and the help", () => {
    const { keymap, ran } = keymapWith();
    press(keymap, "p");
    press(keymap, "o");
    press(keymap, "?");

    expect(ran).toEqual(["presenter", "overview", "help"]);
  });

  it("is case-insensitive, so caps lock does not disarm the deck", () => {
    const { keymap, ran } = keymapWith();
    press(keymap, "B");

    expect(ran).toEqual(["blackout"]);
  });
});

describe("what it refuses to claim", () => {
  it("leaves every modified key to the browser", () => {
    // ⌘R, ⌘←, Ctrl-T. A deck that eats these is a deck the browser no longer
    // works in, and the speaker cannot recover without a mouse.
    for (const modifier of ["metaKey", "ctrlKey", "altKey"] as const) {
      const { keymap, ran } = keymapWith();
      const event = press(keymap, "ArrowRight", { [modifier]: true });

      expect(ran, modifier).toEqual([]);
      expect(event.defaultPrevented, modifier).toBe(false);
    }
  });

  it("holds its fire while someone is typing", () => {
    const { keymap, ran } = keymapWith();
    const input = document.createElement("input");
    document.body.append(input);

    const event = new KeyboardEvent("keydown", { key: "b", cancelable: true, bubbles: true });
    input.dispatchEvent(event);
    keymap.handle(event);

    expect(ran).toEqual([]);
    input.remove();
  });

  it("holds its fire in a contenteditable, which is what the editor uses", () => {
    const { keymap, ran } = keymapWith();
    const editable = document.createElement("div");
    editable.setAttribute("contenteditable", "true");
    document.body.append(editable);

    const event = new KeyboardEvent("keydown", { key: "b", cancelable: true, bubbles: true });
    editable.dispatchEvent(event);
    keymap.handle(event);

    expect(ran).toEqual([]);
    editable.remove();
  });

  it("consumes only the keys it acts on", () => {
    // Claiming an unbound key would break find-in-page and every other thing
    // a browser does with a letter.
    const { keymap } = keymapWith();

    expect(press(keymap, "ArrowRight").defaultPrevented).toBe(true);
    expect(press(keymap, "z").defaultPrevented).toBe(false);
  });

  it("does nothing for a command the page did not supply", () => {
    // A projector page has no timer. Its keys must be inert rather than
    // throwing into a slide.
    const keymap = createKeymap({ commands: { next: () => undefined } });

    expect(() => press(keymap, "t")).not.toThrow();
    expect(press(keymap, "t").defaultPrevented).toBe(false);
  });
});

describe("the table itself", () => {
  it("reaches the demo fallback with a single unmodified key", () => {
    // The whole feature is "one key". A chord, or anything needing a modifier,
    // is a thing to remember at the moment the speaker has nothing spare.
    const binding = DEFAULT_BINDINGS.find((entry) => entry.command === "toggleDemo");

    expect(binding).toBeDefined();
    expect(binding?.keys.every((key) => key.length === 1)).toBe(true);
  });

  it("runs the demo switch when its key is pressed", () => {
    const { keymap, ran } = keymapWith();
    const key = DEFAULT_BINDINGS.find((entry) => entry.command === "toggleDemo")?.keys[0] as string;

    press(keymap, key);
    expect(ran).toEqual(["toggleDemo"]);
  });

  it("leaves the demo key alone on a slide with no demo", () => {
    // Bound to nothing, the key would be swallowed from the browser and give
    // nothing back.
    const keymap = createKeymap({ commands: { next: () => undefined } });

    expect(keymap.bindings().some((entry) => entry.command === "toggleDemo")).toBe(false);
  });

  it("binds no key to two commands", () => {
    // A conflict is silent — one command simply never runs — and nobody finds
    // out until the stage.
    const seen = new Map<string, Command>();

    for (const binding of DEFAULT_BINDINGS) {
      for (const key of binding.keys) {
        const normalised = key.toLowerCase();
        expect(seen.get(normalised), `${key} is bound twice`).toBeUndefined();
        seen.set(normalised, binding.command);
      }
    }
  });

  it("describes every command, so the help can list it", () => {
    for (const binding of DEFAULT_BINDINGS) {
      expect(binding.description, binding.command).toBeTruthy();
      expect(binding.keys.length, binding.command).toBeGreaterThan(0);
    }
  });

  it("lists what it can actually do right now", () => {
    // The help shows the deck's keys, not a catalogue of features this page
    // does not have.
    const keymap = createKeymap({ commands: { next: () => undefined, help: () => undefined } });

    expect(
      keymap
        .bindings()
        .map((binding) => binding.command)
        .sort(),
    ).toEqual(["help", "next"]);
  });

  it("can be rebound without touching the defaults", () => {
    const { ran } = keymapWith();
    const keymap = createKeymap({
      commands: {
        blackout: () => {
          ran.push("blackout");
        },
      },
      bindings: [{ command: "blackout", keys: ["x"], description: "Black out" }],
    });

    press(keymap, "x");
    expect(ran).toEqual(["blackout"]);

    press(keymap, "b");
    expect(ran).toEqual(["blackout"]);
  });
});

describe("showing a key to a person", () => {
  it("writes the keys the way a keyboard shows them", () => {
    expect(formatBinding({ command: "next", keys: [" "], description: "" })).toBe("Space");
    expect(formatBinding({ command: "next", keys: ["ArrowRight"], description: "" })).toBe("→");
    expect(formatBinding({ command: "help", keys: ["?"], description: "" })).toBe("?");
  });

  it("joins alternatives so the help reads as one line", () => {
    expect(
      formatBinding({ command: "next", keys: ["ArrowRight", " ", "PageDown"], description: "" }),
    ).toBe("→ / Space / PageDown");
  });
});
