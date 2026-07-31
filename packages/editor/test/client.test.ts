import { describe, expect, it } from "vite-plus/test";

import { createClient } from "../src/client";
import { CREDENTIAL_HEADER } from "../src/collab";
import { deckOf } from "./support";

function recording() {
  const calls: Array<{ url: string; init: RequestInit | undefined }> = [];
  const deck = deckOf("One");
  const fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
    calls.push({ url, init });
    const body = url.endsWith("measured") ? [] : deck;
    return new Response(JSON.stringify(body), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  }) as typeof globalThis.fetch;

  return { calls, fetch };
}

describe("the editor client on a shared deck", () => {
  it("carries the fragment credential in a header on every request", async () => {
    const sent = recording();
    const secret = "session.secret";
    const client = createClient({
      base: "/editing/",
      fetch: sent.fetch,
      href: `https://deck.example/editing/#s=${secret}`,
    });

    await client.deck();
    await client.apply({ op: "setHeading", slide: 0, text: "Retitled" });
    await client.revert([]);
    await client.measured([]);

    expect(sent.calls.map((call) => call.url)).toEqual([
      "/editing/deck",
      "/editing/edit",
      "/editing/edit",
      "/editing/measured",
    ]);
    for (const call of sent.calls) {
      expect(call.url).not.toContain(secret);
      const headers = call.init?.headers as Record<string, string> | undefined;
      expect(headers?.[CREDENTIAL_HEADER]).toBe(secret);
    }
  });

  it("sends no credential header for the author's ordinary local editor", async () => {
    const sent = recording();
    const client = createClient({
      fetch: sent.fetch,
      href: "http://localhost:5173/__slidx/",
    });

    await client.deck();

    expect(sent.calls[0]!.init?.headers).toEqual({});
  });
});
