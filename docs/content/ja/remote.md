---
title: フォンリモート
summary: 著者がデプロイした Worker 越しに、電話をプレゼンタービューへペアリングします。秘密は URL フラグメントから出ません。
section: reference
order: 10
---

# フォンリモート

著者が動かす Cloudflare Worker 越しに、電話からデッキを進めます。slidx はペアリング
ページ、QR、[オーディエンスチャネル](audience.md) と同じ Worker 上の第二の Durable
Object を書きます。ログインせず、セッションを持たず、デッキが話す Worker を名指すまで
ソケットを出しません。

オプションを外したデッキは、キーボードからそれでも発表できます。ペアリングは拡張です。
リレーのプラグが抜けても、演壇は同じマシンのチャネルでプロジェクタを進めます。

## 1. Worker をデプロイする

オーディエンスチャネルと同じパッケージ、同じ `wrangler.toml`。`@slidxjs/audience` の
インストールから。

```bash
cd node_modules/@slidxjs/audience
wrangler login   # あなたのもので、slidx のものではない
wrangler deploy
```

`wrangler deploy` が印字するオリジンがエンドポイントです。slidx がアカウントを聞くことは
ありません。ペアリングのルートは `/sessions/<id>/socket` です。Q&A ルームとは別の
Durable Object クラスなので、ルームのスラッグがセッションを名指すことも、セッションが
ルームに入ることもありません。

## 2. デッキを向ける

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

`remote` を外すと、ペアリングモジュールは出ません。デフォルトのデッキはそれでも何も
取りません。空のエンドポイントは、外すのと同じです。

## 3. プレゼンタービューを開く

`/slides/presenter/` — ビルドでは `presenter/index.html` — には、Worker を名指したとき
**Phone** ボタンがあります。このウィンドウでペアリングを発行し、QR を描き、リレーを
開きます。コードを読むか、電話でリンクを開きます。

電話のページはデッキ全体で一つの文書で、`/slides/remote/` にあります。その場に留まり、
位置を送ります。停留ごとにナビゲートするページはソケットを落とします。

## 秘密に許されていること

ペアリングの秘密は **URL フラグメント** を旅します。フラグメントはリクエストと一緒に
送られないので、Worker のアクセスログ、プロキシ、ページがリンクする先の referrer には
着きません。

読者は `readPairing` だけです。秘密がすでにクエリ文字列へ漏れた URL は、再生せずに
拒みます。

リモートが言えるのは位置か、位置を求めることだけです。それがメッセージユニオンの全部です。
育つ他の能力はありません。

## slidx がしないこと

Worker を代わりに動かしません。slidx が運用するリレーはこれをサービスにし、それは
slidx が何かという問いであり、非目標がすでに拒んでいます。運用者は著者の Cloudflare
アカウントです。slidx にはいまも HTTP クライアントもトークンストアもありません。
