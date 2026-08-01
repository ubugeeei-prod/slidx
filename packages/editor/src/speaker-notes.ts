/** A focused writing surface for what the speaker says over the current slide. */

import { element } from "./dom";
import type { EditOp } from "./operations";
import type { EditorState } from "./session";
import { applySpeakerNotesStyles } from "./speaker-notes-styles";

export interface SpeakerNotesHandlers {
  run(op: EditOp): void | Promise<void>;
  /** Returns the keyboard to the slide after an explicit notes commit. */
  done?(): void;
}

export interface SpeakerNotesSurface {
  root: HTMLElement;
  render(state: EditorState): void;
  focus(): void;
}

let nextNotes = 0;

export function createSpeakerNotes(handlers: SpeakerNotesHandlers): SpeakerNotesSurface {
  const id = `slidx-speaker-notes-${++nextNotes}`;
  const input = element("textarea", {
    id,
    class: "slidx-speaker-notes-input",
    rows: 3,
    placeholder: "What do you want to remember while this slide is on screen?",
    "aria-label": "Speaker notes",
  }) as HTMLTextAreaElement;
  const stateLabel = element("span", {
    class: "slidx-speaker-notes-state",
    "aria-live": "polite",
  });
  const toggle = element(
    "button",
    {
      type: "button",
      class: "slidx-speaker-notes-toggle",
      "aria-expanded": "true",
      "aria-controls": id,
    },
    [
      element("span", { class: "slidx-speaker-notes-title" }, ["Speaker notes"]),
      element("kbd", { class: "slidx-speaker-notes-key", "aria-hidden": "true" }, ["N"]),
    ],
  ) as HTMLButtonElement;
  const root = element(
    "section",
    { class: "slidx-speaker-notes", "aria-label": "Speaker notes", "data-open": "true" },
    [
      element("header", { class: "slidx-speaker-notes-head" }, [toggle, stateLabel]),
      element("div", { class: "slidx-speaker-notes-body" }, [input]),
    ],
  );
  applySpeakerNotesStyles(root.ownerDocument);

  let slide = -1;
  let authored = "";
  let dirty = false;
  let pending: { slide: number; notes: string } | undefined;
  let latest: EditorState | undefined;

  function opened(value: boolean): void {
    root.dataset.open = String(value);
    toggle.setAttribute("aria-expanded", String(value));
  }

  function status(kind: "clean" | "dirty" | "saving" | "problem", text: string): void {
    stateLabel.dataset.state = kind;
    stateLabel.textContent = text;
  }

  function cleanLabel(state: EditorState | undefined): void {
    if (!state || slide < 0) {
      status("clean", "No slide");
      return;
    }
    const seconds = state.slides[slide]?.estimatedSeconds ?? 0;
    status("clean", seconds > 0 ? `≈ ${showSeconds(seconds)} spoken` : "No notes yet");
  }

  function commit(returnToSlide = false): void {
    if (latest?.canEdit === false) {
      input.value = authored;
      dirty = false;
      cleanLabel(latest);
      if (returnToSlide) handlers.done?.();
      return;
    }
    if (pending?.slide === slide && pending.notes === input.value) {
      if (returnToSlide) handlers.done?.();
      return;
    }
    if (slide < 0 || input.value === authored) {
      dirty = false;
      cleanLabel(latest);
      if (returnToSlide) handlers.done?.();
      return;
    }

    const notes = input.value;
    dirty = false;
    pending = { slide, notes };
    status("saving", "Saving…");
    void handlers.run({ op: "setNotes", slide, notes });
    if (returnToSlide) handlers.done?.();
  }

  toggle.addEventListener("click", () => opened(root.dataset.open !== "true"));
  input.addEventListener("input", () => {
    dirty = input.value !== authored;
    if (dirty) status("dirty", "Unsaved");
    else cleanLabel(latest);
  });
  input.addEventListener("blur", () => commit());
  input.addEventListener("keydown", (event) => {
    if (event.isComposing) return;
    if (event.key === "Escape") {
      event.preventDefault();
      commit(true);
      return;
    }
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      commit(true);
    }
  });

  return {
    root,
    focus() {
      opened(true);
      input.focus();
      input.setSelectionRange(input.value.length, input.value.length);
    },
    render(state) {
      latest = state;
      input.readOnly = state.canEdit === false;
      root.dataset.access = state.canEdit === false ? "read" : "write";
      const nextSlide = state.selection.slide;
      const nextAuthored = state.slides[nextSlide]?.notes.join("\n\n") ?? "";

      if (nextSlide !== slide) {
        const outgoing = dirty && slide >= 0 ? { slide, notes: input.value } : undefined;
        slide = nextSlide;
        authored = nextAuthored;
        dirty = false;
        pending = undefined;
        input.value = nextAuthored;
        input.setAttribute("aria-label", `Speaker notes for slide ${nextSlide + 1}`);
        cleanLabel(state);
        if (outgoing) queueMicrotask(() => void handlers.run({ op: "setNotes", ...outgoing }));
        return;
      }

      if (pending) {
        if (nextAuthored === pending.notes) {
          authored = nextAuthored;
          pending = undefined;
          if (dirty) status("dirty", "Unsaved");
          else {
            input.value = nextAuthored;
            cleanLabel(state);
          }
        } else if (!state.writing) {
          pending = undefined;
          dirty = input.value !== nextAuthored;
          status(state.refusal || state.problem ? "problem" : "dirty", "Not saved");
        }
        return;
      }

      if (!dirty && nextAuthored !== authored) {
        authored = nextAuthored;
        input.value = nextAuthored;
      }
      if (!dirty) cleanLabel(state);
    },
  };
}

function showSeconds(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  return rest === 0 ? `${minutes}m` : `${minutes}m ${rest}s`;
}
