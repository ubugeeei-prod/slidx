/**
 * The entry the presenter page downloads, and no slide does.
 *
 * The other half of `emitted.ts`. That one narrowed what the plugin emits to
 * the names a page imports, which took 8.5KB off a room's download. This one
 * answers the question underneath it: *which* page.
 *
 * A projector and a lectern want different things. Whether the talk will fit is
 * a reading for the speaker, and there is no version of the slide on the wall
 * that needs it — but both pages were handed the same file, so an audience
 * downloaded the timer and the pacing model to run neither.
 *
 * Presentation mode is here for exactly that reason. It arrives with the wake
 * lock, the fullscreen request and a checklist naming the settings a browser
 * cannot touch — none of which a slide has any use for, and all of which a room
 * would have downloaded a week ago.
 *
 * # What is *not* here
 *
 * Anything a slide also imports. `createMirror` and `createNavigator` are on
 * both pages and stay in `emitted.ts`, which the presenter loads as well —
 * duplicating them here would trade a room's bytes for the speaker's twice
 * over, and the shared file is already fetched.
 *
 * `createStopCursor` is the same case for a less obvious reason: only the
 * presenter imports it, but it lives in `stage.ts`, which every staged slide
 * already ships. Moving the name would move the module.
 */

export { assessPace, describePace } from "./pace";
export { detectPlatform, enterPresentation, presentationChecklist } from "./presentation";
export { createTimer, formatDuration } from "./timer";
