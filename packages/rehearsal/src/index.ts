/**
 * The talk you already gave, as data.
 *
 * A deck declares `budget:` per slide, and `slidx_lint` sums those against the
 * slot. That is the plan. This package records the other half — where the time
 * actually went — and diffs the two, per slide, so the answer to a talk running
 * long is "slide 7 took four minutes and was budgeted one" rather than "you
 * ran over", which is the one fact that does not tell a speaker what to cut.
 *
 * A recording is plain JSON, so the same run can be reported now, reported
 * again after a reload, and compared against last week's. That comparison is
 * the point of keeping them: the first rehearsal says where the time went, and
 * the third says whether the cuts worked.
 *
 * Nothing here touches the network, the clock, or storage on its own. The clock
 * is an argument and the recording is a value, so a rehearsal is a
 * specification rather than a suite full of sleeps — and a report outlives the
 * tab that produced it, which matters because most rehearsals end by being
 * abandoned rather than by anyone pressing stop.
 */

export { adviseOn, dominantSlides } from "./advice";
export type { DominantSlide } from "./advice";
export { formatDelta, formatList, formatSpan } from "./duration";
export { createRehearsal, restoreRehearsal, TOLERANCE_MS, TOTAL_TOLERANCE_MS } from "./rehearsal";
export type {
  RecordedSlide,
  Rehearsal,
  RehearsalOptions,
  RehearsalRecording,
  RehearsalSlide,
  RehearsalState,
  RehearsalStatus,
} from "./rehearsal";
export { buildReport } from "./report";
export type {
  BudgetBasis,
  RehearsalReport,
  RehearsalTotals,
  SlideReport,
  SlideVerdict,
  TotalVerdict,
} from "./report";
export { trackRehearsals } from "./trend";
export type { RehearsalTrend, SlideTrend, TrendDirection } from "./trend";
