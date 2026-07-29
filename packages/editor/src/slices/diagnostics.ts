/**
 * What the linter says about the document being edited.
 *
 * Derived state, held here only because recomputing it costs a parse and the
 * editor asks for it on every keystroke. It is *replaced* wholesale rather
 * than merged: a diagnostic that lingers after the line it referred to was
 * deleted is worse than a moment with none, because a speaker stops trusting
 * the panel.
 */

import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

export interface Finding {
  severity: string;
  code: string;
  message: string;
  help?: string;
  slideIndex?: number;
}

export interface DiagnosticsState {
  findings: Finding[];
  /** True while a parse is in flight, so the panel can avoid flickering. */
  checking: boolean;
}

const initialState: DiagnosticsState = { findings: [], checking: false };

export const diagnosticsSlice = createSlice({
  name: "diagnostics",
  initialState,
  reducers: {
    checkStarted(state) {
      state.checking = true;
    },

    checked(state, action: PayloadAction<Finding[]>) {
      state.findings = action.payload;
      state.checking = false;
    },
  },
});

export const { checkStarted, checked } = diagnosticsSlice.actions;

/** Findings for one slide, for the inspector. */
export function findingsFor(state: DiagnosticsState, slide: number): Finding[] {
  return state.findings.filter((finding) => finding.slideIndex === slide);
}

/** True when something would stop a build. */
export function hasBlocking(state: DiagnosticsState): boolean {
  return state.findings.some((finding) => finding.severity === "error");
}
