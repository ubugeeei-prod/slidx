/**
 * Sharing a dev server, treated as the security boundary it is.
 *
 * These routes write to the author's files. Everything below is a statement
 * about who may reach them, and the two that matter most are the ones a
 * convenient default would get wrong: read-only is not a flag on the edit
 * secret, and nothing is shared at all until somebody asks.
 */

import { describe, expect, it } from "vite-plus/test";

import {
  createSharing,
  isLoopback,
  Grant,
  SHARE_EDIT_VARIABLE,
  SHARE_VARIABLE,
} from "../src/share";

/** A pairing of the shape `slidx dev --crdt` mints. */
const SESSION = "0123456789abcdef";
const READ = "00112233445566778899aabbccddeeff";
const EDIT = "ffeeddccbbaa99887766554433221100";

const READ_LINK = `${SESSION}.${READ}`;
const EDIT_LINK = `${SESSION}.${EDIT}`;

const ELSEWHERE = "192.168.1.42";

function sharing(environment: Record<string, string> = {}) {
  return createSharing(environment as NodeJS.ProcessEnv);
}

function shared() {
  return sharing({ [SHARE_VARIABLE]: READ_LINK, [SHARE_EDIT_VARIABLE]: EDIT_LINK });
}

describe("a dev server nobody asked to share", () => {
  it("is not sharing, and answers every request in full", () => {
    // The default has to be that this behaves exactly as it always has. A
    // plugin used without slidx never sets these.
    const off = sharing();

    expect(off.on).toBe(false);
    expect(off.grant(undefined, "127.0.0.1")).toBe(Grant.Write);
  });

  it("treats a half-configured secret as no secret rather than as one nobody can match", () => {
    // Sharing that silently refused everything would look exactly like a
    // network problem, in a room where a network problem is the first guess.
    expect(sharing({ [SHARE_VARIABLE]: "" }).on).toBe(false);
    expect(sharing({ [SHARE_VARIABLE]: "not-a-pairing" }).on).toBe(false);
  });
});

describe("a shared dev server", () => {
  it("answers the author's own machine without a secret", () => {
    // The bookmark the author already had keeps working the moment they share.
    expect(shared().grant(undefined, "127.0.0.1")).toBe(Grant.Write);
    expect(shared().grant(undefined, "::1")).toBe(Grant.Write);
    expect(shared().grant(undefined, "::ffff:127.0.0.1")).toBe(Grant.Write);
  });

  it("refuses anything from elsewhere that carries no secret", () => {
    expect(shared().grant(undefined, ELSEWHERE)).toBe(Grant.None);
    expect(shared().grant("", ELSEWHERE)).toBe(Grant.None);
  });

  it("gives the share link reading and nothing more", () => {
    // The whole point. Somebody sharing their screen at a conference has not
    // handed out the right to rewrite their talk.
    expect(shared().grant(READ_LINK, ELSEWHERE)).toBe(Grant.Read);
  });

  it("gives editing only to a second secret the viewer was never sent", () => {
    // Not a flag on the same token — a different sixteen bytes. Read-only is
    // true by construction rather than by policy.
    expect(shared().grant(EDIT_LINK, ELSEWHERE)).toBe(Grant.Write);
  });

  it("shares reading without an edit secret existing at all", () => {
    const readOnly = sharing({ [SHARE_VARIABLE]: READ_LINK });

    expect(readOnly.grant(READ_LINK, ELSEWHERE)).toBe(Grant.Read);
    expect(readOnly.grant(EDIT_LINK, ELSEWHERE)).toBe(Grant.None);
  });

  it("refuses a secret for another session, however right the secret is", () => {
    expect(shared().grant(`fedcba9876543210.${READ}`, ELSEWHERE)).toBe(Grant.None);
  });

  it("refuses a secret that is a prefix of the right one", () => {
    // Guarded by nothing but its own length, so a comparison that stopped early
    // would turn sixteen bytes into sixteen guesses.
    expect(shared().grant(`${SESSION}.${READ.slice(0, 30)}`, ELSEWHERE)).toBe(Grant.None);
  });

  it("refuses a credential that is not a pair of hex tokens", () => {
    // The rule lives in `readPairing`, which is also what the phone remote
    // reads — one answer in the repository rather than two.
    for (const nonsense of [
      "nope",
      `${SESSION}.`,
      `${SESSION}.${READ}.extra`,
      "../../etc/passwd",
    ]) {
      expect(shared().grant(nonsense, ELSEWHERE)).toBe(Grant.None);
    }
  });
});

describe("the link slidx prints and the credential this reads", () => {
  it("accepts the exact fragment `slidx dev --crdt` puts in a URL", () => {
    // The cross-language pin. `crates/slidx_cli/src/dev/share.rs` asserts it
    // prints this URL; this asserts the dev server honours what comes out of it.
    // Two spellings of one shape is only tolerable while both are named.
    const printed =
      "http://192.168.1.42:5173/__slidx/#s=0123456789abcdef.00112233445566778899aabbccddeeff";
    const fragment = printed.slice(printed.indexOf("#s=") + 3);

    expect(shared().grant(fragment, ELSEWHERE)).toBe(Grant.Read);
  });
});

describe("recognising the machine the dev server is on", () => {
  it("knows every spelling of loopback Node reports", () => {
    // A dual-stack socket reports an IPv4 peer as `::ffff:127.0.0.1`. Missing
    // that would lock the author out of their own editor when they shared it.
    for (const address of ["127.0.0.1", "127.1.2.3", "::1", "::ffff:127.0.0.1"]) {
      expect(isLoopback(address)).toBe(true);
    }
  });

  it("does not mistake anything else for it", () => {
    for (const address of [
      "192.168.1.42",
      "10.0.0.2",
      "::ffff:192.168.1.42",
      "1.127.0.0",
      undefined,
    ]) {
      expect(isLoopback(address)).toBe(false);
    }
  });
});
