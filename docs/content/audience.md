---
title: Audience channel
summary: Opt a deck into moderated Q&A and reactions, on a Worker the author deploys.
section: reference
order: 9
---

# Audience channel

Questions and reactions from the room, on a Cloudflare Worker _you_ run. slidx
writes the Worker and the client; it does not log in, does not hold a
credential, and does not inject either until the deck names the Worker it
should talk to. A deck that leaves the option out still presents with the
network off.

## 1. Deploy the Worker

The package ships a `wrangler.toml` whose `main` is the Durable Object that
owns a room. From an install of `@slidxjs/audience`:

```bash
cd node_modules/@slidxjs/audience
wrangler login   # yours, not slidx's
wrangler deploy
```

`wrangler deploy` prints the origin, something like
`https://slidx-audience.<account>.workers.dev`. That string is the endpoint.
slidx never asks for the account, and the toml names no token.

## 2. Point the deck at it

```ts
import { defineConfig } from "vite";
import { slidx } from "@slidxjs/vite-plugin";

export default defineConfig({
  plugins: [
    slidx({
      audience: {
        endpoint: "https://slidx-audience.example.workers.dev",
        room: "zero-js",
        // Presenter pages only. Omit it and the speaker joins as a participant.
        hostKey: "a-key-you-chose",
      },
    }),
  ],
});
```

`room` is the Durable Object name. Lowercase letters, digits, and hyphens;
the same rules as a URL slug. A capital letter is a different room, and half
the audience would sit in the empty one, so slidx will not inject a client
for a name the Worker would refuse.

`hostKey` is the speaker's. It is written onto presenter pages and never onto
an audience slide. It is not an account, and slidx does not store it anywhere
else.

Leave `audience` out and no client is emitted. The default deck still fetches
nothing.

## What slidx will not do

It will not run the Worker for you. A relay slidx operated would make this a
service, which is a question about what slidx is, and the non-goals already
refuse it. The operator is the author's Cloudflare account.
