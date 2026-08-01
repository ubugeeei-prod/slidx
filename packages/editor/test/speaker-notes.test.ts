/** The speaking surface beside the slide, driven by semantic note edits. */

import { afterEach, describe, expect, it } from "vite-plus/test";

import type { EditOp } from "../src/operations";
import type { EditorState } from "../src/session";
import { createSpeakerNotes } from "../src/speaker-notes";
import { deckOf } from "./support";

afterEach(() => document.body.replaceChildren());

function stateOf(
  selected = 0,
  notes = ["Open with the outcome.", "Name the tradeoff."],
): EditorState {
  const built = deckOf("Opening", "Decision");
  built.deck.slides.forEach((slide, index) => {
    slide.notes = notes[index] ? [notes[index]!] : [];
    slide.estimatedSeconds = index === 0 ? 12 : 8;
  });

  return {
    source: built.source,
    spans: built.spans,
    title: built.deck.title,
    slides: built.deck.slides,
    layouts: built.deck.layouts,
    activeTheme: built.deck.activeTheme,
    themeLocked: built.deck.themeLocked,
    themes: built.deck.themes,
    transitions: built.deck.transitions,
    durationSeconds: built.deck.durationSeconds,
    diagnostics: built.deck.diagnostics,
    selection: { slide: selected },
    viewers: [],
    canUndo: false,
    canRedo: false,
    writing: false,
  };
}

function open() {
  const ops: EditOp[] = [];
  let done = 0;
  const notes = createSpeakerNotes({
    run: (op) => {
      ops.push(op);
    },
    done: () => {
      done += 1;
    },
  });
  document.body.append(notes.root);
  return { notes, ops, done: () => done };
}

function inputIn(root: HTMLElement): HTMLTextAreaElement {
  return root.querySelector<HTMLTextAreaElement>(".slidx-speaker-notes-input")!;
}

describe("speaker notes", () => {
  it("keeps the current slide visible while showing its message and pipeline timing", () => {
    const { notes } = open();
    notes.render(stateOf());

    const input = inputIn(notes.root);
    expect(input.value).toBe("Open with the outcome.");
    expect(input.getAttribute("aria-label")).toBe("Speaker notes for slide 1");
    expect(notes.root.querySelector(".slidx-speaker-notes-state")!.textContent).toBe(
      "≈ 12s spoken",
    );

    notes.render(stateOf(1));
    expect(input.value).toBe("Name the tradeoff.");
    expect(input.getAttribute("aria-label")).toBe("Speaker notes for slide 2");
  });

  it("writes one operation on blur and none for an untouched message", () => {
    const { notes, ops } = open();
    notes.render(stateOf());
    const input = inputIn(notes.root);

    input.dispatchEvent(new Event("blur"));
    expect(ops).toEqual([]);

    input.value = "Lead with the decision.";
    input.dispatchEvent(new Event("input"));
    expect(notes.root.querySelector(".slidx-speaker-notes-state")!.textContent).toBe("Unsaved");
    input.dispatchEvent(new Event("blur"));

    expect(ops).toEqual([{ op: "setNotes", slide: 0, notes: "Lead with the decision." }]);
  });

  it("keeps an in-flight draft until the pipeline returns the authored note", () => {
    const { notes } = open();
    notes.render(stateOf());
    const input = inputIn(notes.root);
    input.value = "Lead with the decision.";
    input.dispatchEvent(new Event("input"));
    input.dispatchEvent(new Event("blur"));

    notes.render({ ...stateOf(), writing: true });
    expect(input.value).toBe("Lead with the decision.");
    expect(notes.root.querySelector(".slidx-speaker-notes-state")!.textContent).toBe("Saving…");

    const saved = stateOf(0, ["Lead with the decision.", "Name the tradeoff."]);
    saved.slides[0]!.estimatedSeconds = 9;
    notes.render(saved);
    expect(notes.root.querySelector(".slidx-speaker-notes-state")!.textContent).toBe("≈ 9s spoken");
  });

  it("commits the outgoing draft if selection changes before blur", async () => {
    const { notes, ops } = open();
    notes.render(stateOf());
    const input = inputIn(notes.root);
    input.value = "Do not lose this thought.";
    input.dispatchEvent(new Event("input"));

    notes.render(stateOf(1));
    await Promise.resolve();

    expect(ops).toEqual([{ op: "setNotes", slide: 0, notes: "Do not lose this thought." }]);
    expect(input.value).toBe("Name the tradeoff.");
  });

  it("collapses without losing content and the N command reopens it at the caret", () => {
    const { notes } = open();
    notes.render(stateOf());
    const toggle = notes.root.querySelector<HTMLButtonElement>(".slidx-speaker-notes-toggle")!;
    const input = inputIn(notes.root);

    toggle.click();
    expect(notes.root.dataset.open).toBe("false");
    expect(toggle.getAttribute("aria-expanded")).toBe("false");

    notes.focus();
    expect(notes.root.dataset.open).toBe("true");
    expect(document.activeElement).toBe(input);
    expect(input.selectionStart).toBe(input.value.length);
  });

  it("commits with the primary Enter gesture and returns focus to the slide", () => {
    const { notes, ops, done } = open();
    notes.render(stateOf());
    const input = inputIn(notes.root);
    input.value = "Finish on the promise.";
    input.dispatchEvent(new Event("input"));

    const event = new KeyboardEvent("keydown", {
      key: "Enter",
      metaKey: true,
      bubbles: true,
      cancelable: true,
    });
    input.dispatchEvent(event);
    // Returning focus to the slide blurs the textarea in the mounted editor.
    input.dispatchEvent(new Event("blur"));

    expect(event.defaultPrevented).toBe(true);
    expect(ops).toEqual([{ op: "setNotes", slide: 0, notes: "Finish on the promise." }]);
    expect(done()).toBe(1);
  });

  it("does not replace newer typing when an earlier save finishes", () => {
    const { notes } = open();
    notes.render(stateOf());
    const input = inputIn(notes.root);
    input.value = "First draft.";
    input.dispatchEvent(new Event("input"));
    input.dispatchEvent(new Event("blur"));

    input.value = "A better draft written while saving.";
    input.dispatchEvent(new Event("input"));
    notes.render(stateOf(0, ["First draft.", "Name the tradeoff."]));

    expect(input.value).toBe("A better draft written while saving.");
    expect(notes.root.querySelector(".slidx-speaker-notes-state")!.textContent).toBe("Unsaved");
  });

  it("shows notes as readable text without offering a write through a view-only link", () => {
    const { notes, ops } = open();
    notes.render({ ...stateOf(), canEdit: false });
    const input = inputIn(notes.root);

    expect(input.readOnly).toBe(true);
    expect(input.value).toBe("Open with the outcome.");
    expect(notes.root.dataset.access).toBe("read");

    input.value = "A change the browser should not send.";
    input.dispatchEvent(new Event("blur"));
    expect(input.value).toBe("Open with the outcome.");
    expect(ops).toEqual([]);
  });
});
