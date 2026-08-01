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
    const body = url.endsWith("measured")
      ? []
      : url.endsWith("share")
        ? { enabled: true, read: "https://deck.example/__slidx/#s=viewer" }
        : url.endsWith("media")
          ? { kind: "image", src: "/slides/assets/chart.png", alt: "chart" }
          : deck;
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
    await client.upload(new File(["image"], "日本 chart.png", { type: "image/png" }));
    await client.measured([]);

    expect(sent.calls.map((call) => call.url)).toEqual([
      "/editing/deck",
      "/editing/edit",
      "/editing/edit",
      "/editing/media",
      "/editing/measured",
    ]);
    for (const call of sent.calls) {
      expect(call.url).not.toContain(secret);
      const headers = call.init?.headers as Record<string, string> | undefined;
      expect(headers?.[CREDENTIAL_HEADER]).toBe(secret);
    }
    expect(sent.calls[3]!.init?.headers).toMatchObject({
      "content-type": "image/png",
      "x-slidx-name": encodeURIComponent("日本 chart.png"),
    });
    expect(sent.calls[3]!.init?.body).toBeInstanceOf(File);
  });

  it("sends no credential header for the author's ordinary local editor", async () => {
    const sent = recording();
    const client = createClient({
      fetch: sent.fetch,
      href: "http://localhost:5173/__slidx/",
    });

    await client.deck();
    await client.sharing!();

    expect(sent.calls[0]!.init?.headers).toEqual({});
    expect(sent.calls[1]).toMatchObject({ url: "/__slidx/share", init: { headers: {} } });
  });

  it("turns an invalid share capability into an open failure the workspace can name", async () => {
    const client = createClient({
      fetch: (() =>
        Promise.resolve(
          new Response(JSON.stringify({ message: "This deck is shared by link." }), {
            status: 403,
            headers: { "content-type": "application/json" },
          }),
        )) as typeof globalThis.fetch,
      href: "https://deck.example/__slidx/#s=invalid",
    });

    await expect(client.deck()).rejects.toThrow("shared by link");
  });

  it("treats the local-only handoff route as absent in an invited browser", async () => {
    let calls = 0;
    const client = createClient({
      fetch: (() => {
        calls += 1;
        return Promise.resolve(new Response("{}", { status: 403 }));
      }) as typeof globalThis.fetch,
      href: "https://deck.example/__slidx/#s=viewer",
    });

    await expect(client.sharing!()).resolves.toBeNull();
    expect(calls).toBe(0);
  });
});
