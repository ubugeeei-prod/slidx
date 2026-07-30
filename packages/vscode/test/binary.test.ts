import { describe, expect, it } from "vite-plus/test";

import { installRoot, isFound, nowhere, resolve, type Machine, type NotFound } from "../src/binary";

/**
 * A machine with nothing installed anywhere.
 *
 * Every case below adds exactly what it is about, so a test that passes for the
 * wrong reason has nowhere to hide.
 */
function bare(overrides: Partial<Machine> = {}): Machine {
  return {
    env: {},
    windows: false,
    executable: () => false,
    ...overrides,
  };
}

/** A machine where these paths, and only these, are runnable. */
function holding(paths: string[], overrides: Partial<Machine> = {}): Machine {
  return bare({ executable: (path) => paths.includes(path), ...overrides });
}

function command(machine: Machine): string | undefined {
  const found = resolve(machine);
  return isFound(found) ? found.command : undefined;
}

describe("finding the slidx binary", () => {
  it("takes the configured path over everything else, and does not second-guess it", () => {
    // An author who typed a path wants to hear about that path. A setting that
    // silently fell back to something else is how somebody spends an hour
    // debugging the wrong binary.
    const machine = holding(["/usr/local/bin/slidx"], { configured: "/opt/built/slidx" });

    expect(command(machine)).toBe("/opt/built/slidx");
  });

  it("ignores a configured path that is only whitespace", () => {
    // What the setting looks like after somebody clears it in the UI.
    const machine = holding(["/usr/local/bin/slidx"], {
      configured: "  ",
      env: { PATH: "/usr/local/bin" },
    });

    expect(command(machine)).toBe("/usr/local/bin/slidx");
  });

  it("prefers whatever is on the PATH, because that is what the author's terminal runs", () => {
    // `slidx version use` and a .slidx-version pin both act on the PATH. An
    // editor reaching past it would lint a deck with a different slidx than the
    // one `slidx lint` runs in the same project.
    const machine = holding(["/home/somebody/.slidx/bin/slidx", "/usr/local/bin/slidx"], {
      env: { PATH: "/usr/local/bin:/usr/bin", HOME: "/home/somebody" },
    });

    expect(command(machine)).toBe("/usr/local/bin/slidx");
    expect(isFound(resolve(machine)) && resolve(machine)).toMatchObject({ origin: "path" });
  });

  it("walks the PATH in the order the PATH gives, so a shadowing install still wins", () => {
    // The same answer the shell would give. An extension that picked a later
    // entry would disagree with `slidx version current`, which is the one
    // command anybody would use to check.
    const machine = holding(["/first/slidx", "/second/slidx"], {
      env: { PATH: "/first:/second" },
    });

    expect(command(machine)).toBe("/first/slidx");
  });

  it("falls back to the install directory, because an editor is not a login shell", () => {
    // A GUI application started from a dock gets the session manager's PATH,
    // which on macOS is not the one in anybody's profile.
    const machine = holding(["/home/somebody/.slidx/bin/slidx"], {
      env: { HOME: "/home/somebody" },
    });

    expect(command(machine)).toBe("/home/somebody/.slidx/bin/slidx");
    const found = resolve(machine);
    expect(isFound(found) && found.origin).toBe("install");
  });

  it("reports every place it looked when there is nothing to start", () => {
    // "slidx not found" is the message that sends somebody to reinstall a
    // binary they already have.
    const found = resolve(bare({ env: { PATH: "/usr/bin", HOME: "/home/somebody" } }));

    expect(isFound(found)).toBe(false);
    expect((found as NotFound).looked).toEqual([
      "/usr/bin/slidx",
      "/home/somebody/.slidx/bin/slidx",
    ]);
  });

  it("names the install command and the setting when it found nothing", () => {
    const message = nowhere({ looked: ["/home/somebody/.slidx/bin/slidx"] });

    expect(message).toContain("npm i -g slidx");
    expect(message).toContain("slidx.path");
    expect(message).toContain("slidx version current");
  });

  it("says nothing about where it looked when it had nowhere to look", () => {
    // A container with no HOME and no PATH. Naming an empty list would read as
    // a bug in the extension rather than as an environment with nothing in it.
    expect(nowhere({ looked: [] })).not.toContain("Looked");
  });
});

describe("the install root, which is where slidx puts itself", () => {
  it("is $SLIDX_HOME when that is set, over every convention", () => {
    // The escape hatch install.sh documents. It has to outrank the conventions
    // or it is not one.
    const machine = bare({
      env: { SLIDX_HOME: "/opt/slidx", XDG_DATA_HOME: "/home/somebody/.local/share" },
    });

    expect(installRoot(machine)).toBe("/opt/slidx");
  });

  it("is $XDG_DATA_HOME/slidx where the platform has that convention", () => {
    const machine = bare({ env: { XDG_DATA_HOME: "/home/somebody/.local/share" } });

    expect(installRoot(machine)).toBe("/home/somebody/.local/share/slidx");
  });

  it("is ~/.slidx otherwise", () => {
    expect(installRoot(bare({ env: { HOME: "/home/somebody" } }))).toBe("/home/somebody/.slidx");
  });

  it("treats an exported-but-empty variable as absent rather than as the filesystem root", () => {
    // How a variable looks after a shell script unset it badly. `/slidx` would
    // be a surprising place to go looking.
    const machine = bare({
      env: { SLIDX_HOME: "", XDG_DATA_HOME: "", HOME: "/home/somebody" },
    });

    expect(installRoot(machine)).toBe("/home/somebody/.slidx");
  });

  it("is a Windows path on Windows, and never a dotfile", () => {
    // A dot-prefixed directory hides nothing there. It is a folder with an odd
    // name in the middle of somebody's home directory.
    const machine = bare({
      windows: true,
      env: { LOCALAPPDATA: "C:\\Users\\somebody\\AppData\\Local" },
    });

    expect(installRoot(machine)).toContain("AppData");
    expect(installRoot(machine)).not.toContain(".slidx");
  });

  it("does not consult XDG_DATA_HOME on Windows", () => {
    // Not a convention that platform has. Honouring it would put slidx
    // somewhere no other Windows tool looks.
    const machine = bare({
      windows: true,
      env: { XDG_DATA_HOME: "/wherever", USERPROFILE: "C:\\Users\\somebody" },
    });

    expect(installRoot(machine)).not.toContain("wherever");
  });

  it("falls back to the user profile on a Windows session with no LOCALAPPDATA", () => {
    const machine = bare({ windows: true, env: { USERPROFILE: "C:\\Users\\somebody" } });

    expect(installRoot(machine)).toContain("somebody");
  });

  it("is nowhere at all on a machine with no home directory", () => {
    // A container running as a user with no passwd entry. Nothing to look in
    // is a real answer, and better than inventing a relative path.
    expect(installRoot(bare())).toBeUndefined();
  });
});

describe("on Windows, where a bare name is not a file name", () => {
  it("looks for the executable extensions PATHEXT names", () => {
    // Every release puts `slidx.exe` there, and a lookup that only tried
    // `slidx` would find nothing on the one platform that ships it that way.
    const machine = holding(["C:\\tools\\slidx.exe"], {
      windows: true,
      env: { PATH: "C:\\tools", PATHEXT: ".COM;.EXE;.BAT" },
    });

    expect(command(machine)).toBe("C:\\tools\\slidx.exe");
  });

  it("has its own default list when PATHEXT is unset", () => {
    const machine = holding(["C:\\tools\\slidx.exe"], {
      windows: true,
      env: { PATH: "C:\\tools" },
    });

    expect(command(machine)).toBe("C:\\tools\\slidx.exe");
  });

  it("reads Path as well as PATH, because Windows spells it either way", () => {
    const machine = holding(["C:\\tools\\slidx.exe"], {
      windows: true,
      env: { Path: "C:\\tools" },
    });

    expect(command(machine)).toBe("C:\\tools\\slidx.exe");
  });
});
