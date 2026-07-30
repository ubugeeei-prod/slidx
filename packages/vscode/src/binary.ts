/**
 * Where `slidx` is on this machine.
 *
 * This is the part of an editor plugin that actually fails. A language server
 * that never starts looks identical to one that has nothing to say, so an
 * extension that cannot find its binary has to say which places it looked.
 *
 * ## The order, and where it comes from
 *
 * slidx installs into `$SLIDX_HOME`, else `$XDG_DATA_HOME/slidx`, else
 * `~/.slidx` — and `%LOCALAPPDATA%\slidx` on Windows, where XDG is not a
 * convention that platform has. `install.sh` writes there, `slidx version`
 * manages there, and `crates/slidx_cli/src/home.rs` is where that order is
 * decided.
 *
 * Nothing in TypeScript can read a Rust constant, so this file restates it and
 * a Rust test reads this file back — see
 * `crates/slidx_cli/tests/language_server.rs`. Two spellings of one answer is
 * tolerable only while both are pinned, and the failure it prevents is the
 * expensive kind: a version manager silently not managing the binary an editor
 * is running.
 *
 * ## PATH comes first, and that is deliberate
 *
 * Whatever is on the PATH is what the author's own terminal runs, which is
 * what `slidx version use` and a `.slidx-version` pin both act on. Reaching
 * past it into the install directory would mean an editor quietly running a
 * different slidx from the one `slidx lint` runs in the same project — which is
 * the whole failure `slidx version current` exists to report.
 *
 * The install directory is the fallback rather than the answer because an
 * editor is not a login shell. A GUI application started from a dock has
 * whatever PATH the session manager gave it, which on macOS is famously not the
 * one in anybody's profile.
 */

import { accessSync, constants } from "node:fs";
import { posix, win32, type PlatformPath } from "node:path";

/** The binary every editor plugin starts, and the CLI everything else runs. */
export const BINARY = "slidx";

/**
 * Where the install root is read from, in order, on everything but Windows.
 *
 * `HOME` is last and is not itself the root — `~/.slidx` is. The other two name
 * the root directly, minus the `slidx` suffix `XDG_DATA_HOME` takes.
 */
export const HOME_VARIABLES = ["SLIDX_HOME", "XDG_DATA_HOME", "HOME"] as const;

/**
 * And on Windows, where `XDG_DATA_HOME` is not consulted at all.
 *
 * A dot-prefixed directory hides nothing there; it is a folder with an odd name
 * in the middle of somebody's home directory. `USERPROFILE` is the fallback for
 * a session with no `LOCALAPPDATA`.
 */
export const WINDOWS_HOME_VARIABLES = ["SLIDX_HOME", "LOCALAPPDATA", "USERPROFILE"] as const;

/** The directory that goes on the PATH, under the install root. */
export const BIN_DIRECTORY = "bin";

/** How `slidx` was found, so the extension can say so when asked. */
export type Origin = "setting" | "path" | "install";

export interface Found {
  readonly command: string;
  readonly origin: Origin;
}

export interface NotFound {
  /** Every absolute path that was tried, in the order they were tried. */
  readonly looked: readonly string[];
  /** True when the PATH had anything on it to search. */
  readonly searchedPath: boolean;
  /** The install directory that was searched, if this machine has one. */
  readonly installDirectory?: string | undefined;
}

export type Resolution = Found | NotFound;

export function isFound(resolution: Resolution): resolution is Found {
  return "command" in resolution;
}

/**
 * Path arithmetic for the machine being described, not for the one running.
 *
 * In production these are the same and `node:path` would do. They are not the
 * same in a test, and the difference is not cosmetic: `PATH` is split on `;` on
 * Windows and on `:` everywhere else, so a posix split of `C:\tools` finds a
 * directory called `C`.
 */
function paths(machine: Machine): PlatformPath {
  return machine.windows ? win32 : posix;
}

/** The readings this decision depends on, so every branch is reachable. */
export interface Machine {
  readonly env: Readonly<Record<string, string | undefined>>;
  readonly windows: boolean;
  /** True for a path that exists and can be executed. */
  readonly executable: (path: string) => boolean;
  /** What the author put in `slidx.path`, if anything. */
  readonly configured?: string | undefined;
}

/**
 * Finds the binary, or reports everywhere it was not.
 *
 * An absolute `slidx.path` is taken as given and is *not* checked for
 * existence: an author who typed a path wants to hear about that path, and a
 * setting silently falling back to something else is how somebody debugs the
 * wrong binary for an hour.
 */
export function resolve(machine: Machine): Resolution {
  const configured = machine.configured?.trim();
  if (configured !== undefined && configured !== "") {
    return { command: configured, origin: "setting" };
  }

  const { join } = paths(machine);
  const looked: string[] = [];

  const onThePath = onPath(machine);
  for (const candidate of onThePath) {
    looked.push(candidate);
    if (machine.executable(candidate)) return { command: candidate, origin: "path" };
  }

  const root = installRoot(machine);
  const installDirectory = root === undefined ? undefined : join(root, BIN_DIRECTORY);

  if (installDirectory !== undefined) {
    for (const name of executableNames(machine)) {
      const candidate = join(installDirectory, name);
      looked.push(candidate);
      if (machine.executable(candidate)) return { command: candidate, origin: "install" };
    }
  }

  return { looked, searchedPath: onThePath.length > 0, installDirectory };
}

/**
 * The install root, resolved the way `slidx_cli::home` resolves it.
 *
 * An exported-but-empty variable is how one looks after a shell script unset it
 * badly, and treating it as a request to install into the filesystem root would
 * be a surprising way to find that out — so empty is the same as absent here
 * too.
 */
export function installRoot(machine: Machine): string | undefined {
  const { join } = paths(machine);
  const read = (name: string): string | undefined => {
    const value = machine.env[name];
    return value !== undefined && value !== "" ? value : undefined;
  };

  const explicit = read("SLIDX_HOME");
  if (explicit !== undefined) return explicit;

  if (machine.windows) {
    const base = read("LOCALAPPDATA") ?? read("USERPROFILE");
    return base === undefined ? undefined : join(base, "slidx");
  }

  const data = read("XDG_DATA_HOME");
  if (data !== undefined) return join(data, "slidx");

  const home = read("HOME");
  return home === undefined ? undefined : join(home, ".slidx");
}

/** Every candidate on the PATH, in PATH order. */
function onPath(machine: Machine): string[] {
  const { join, delimiter } = paths(machine);
  // Windows spells the variable either way depending on who set it, and its
  // environment block is case-insensitive while a JavaScript object is not.
  const path = machine.env["PATH"] ?? machine.env["Path"] ?? "";
  const names = executableNames(machine);

  return path
    .split(delimiter)
    .filter((entry) => entry !== "")
    .flatMap((entry) => names.map((name) => join(entry, name)));
}

/**
 * What the binary is called on this platform.
 *
 * Windows resolves a bare name through `PATHEXT`, and a lookup that only tried
 * `slidx` would miss the `slidx.exe` every release puts there.
 */
function executableNames(machine: Machine): string[] {
  if (!machine.windows) return [BINARY];

  const extensions = (machine.env["PATHEXT"] ?? ".COM;.EXE;.BAT;.CMD")
    .split(";")
    .map((extension) => extension.trim())
    .filter((extension) => extension !== "");

  return extensions.map((extension) => `${BINARY}${extension.toLowerCase()}`);
}

/** Reads the real machine. */
export function machine(configured?: string | undefined): Machine {
  return {
    env: process.env,
    windows: process.platform === "win32",
    executable: canExecute,
    configured,
  };
}

function canExecute(path: string): boolean {
  try {
    // X_OK is meaningless on Windows and always passes, which is correct: the
    // question there is whether the file is there at all.
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

/**
 * What to tell an author when there is no slidx to start.
 *
 * Names the places that were tried, because "slidx not found" is the message
 * that sends somebody to reinstall a binary they already have — the usual
 * answer is that an editor started from a dock never saw their profile's PATH.
 *
 * The two places are named separately rather than as one list of candidate
 * files: which of them came up empty is the whole diagnosis, and a wall of
 * paths is not something anybody reads out of a notification.
 */
export function nowhere(resolution: NotFound): string {
  const places: string[] = [];
  if (resolution.searchedPath) places.push("on your PATH");
  if (resolution.installDirectory !== undefined) places.push(`in ${resolution.installDirectory}`);

  const tried = places.length > 0 ? ` Looked ${places.join(" and ")}.` : "";

  return (
    `slidx: no ${BINARY} binary found, so the language server cannot start.${tried}` +
    " Install it with `npm i -g slidx`, or set `slidx.path` to the binary you have —" +
    " `slidx version current` in a terminal prints where that is."
  );
}
