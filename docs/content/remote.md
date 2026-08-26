---
title: Phone remote
summary: Pair a phone to the presenter view over a Worker the author deploys. The secret never leaves the URL fragment.
section: reference
order: 10
---

# Phone remote

Drive the deck from a phone, on a Cloudflare Worker _you_ run. slidx writes the
pairing page, the QR, and a second Durable Object on the same Worker as the
[audience channel](audience.md). It does not log in, does not hold the session,
and does not emit a socket unless the deck names the Worker it should talk to.

A deck that leaves the option out still presents from the keyboard. Pairing is
an enhancement: when the relay's plug is pulled, the lectern still drives the
projector over the same-machine channel.

## 1. Deploy the Worker

The same package and the same `wrangler.toml` as the audience channel. From an
install of `@slidxjs/audience`:

```bash
cd node_modules/@slidxjs/audience
wrangler login   # yours, not slidx's
wrangler deploy
```

The origin `wrangler deploy` prints is the endpoint. slidx never asks for the
account. The pairing route is `/sessions/<id>/socket` — a different Durable
Object class from a Q&A room, so a room slug can never name a session and a
session can never join a room.

## 2. Point the deck at it

```ts
import { defineConfig } from "vite";
import { slidx } from "@slidxjs/vite-plugin";

export default defineConfig({
  plugins: [
    slidx({
      remote: {
        endpoint: "https://slidx-audience.example.workers.dev",
      },
    }),
  ],
});
```

Leave `remote` out and no pairing module is emitted. The default deck still
fetches nothing. An empty endpoint is the same as leaving it out.

## 3. Open the presenter view

`/slides/presenter/` — or `presenter/index.html` in the build — has a **Phone**
button when a Worker was named. It mints a pairing in this window, draws a QR,
and opens the relay. Scan the code, or open the link on the phone.

The phone page is one document for the whole deck, at `/slides/remote/`. It
stays put and sends positions. A page that navigated on every step would drop
the socket.

## What the secret is allowed to do

The pairing secret travels in the **URL fragment**. A fragment is not sent with
the request, so it does not land in the Worker's access log, in a proxy, or in
the referrer of anything the page links to.

`readPairing` is the only reader. A URL that already leaked the secret into a
query string is refused rather than replayed.

The remote can say a position, or ask for one. That is the whole message
union. There is no other capability to grow into.

## What slidx will not do

It will not run the Worker for you. A relay slidx operated would make this a
service, which is a question about what slidx is, and the non-goals already
refuse it. The operator is the author's Cloudflare account. slidx still has no
HTTP client and no token store.
