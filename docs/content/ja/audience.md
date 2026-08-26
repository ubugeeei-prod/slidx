---
title: オーディエンスチャネル
summary: 著者がデプロイした Worker 上で、モデレートされた Q&A とリアクションをデッキに足します。
section: reference
order: 9
---

# オーディエンスチャネル

会場からの質問とリアクションを、_あなたが_ 動かす Cloudflare Worker の上で。slidx は
Worker とクライアントを書きます。ログインせず、認証情報を持たず、デッキが話す Worker を
名指すまでどちらも注入しません。オプションを外したデッキは、ネットワークなしでも
発表できます。

## 1. Worker をデプロイする

パッケージは、ルームを所有する Durable Object を `main` にした `wrangler.toml` を
出荷します。`@slidxjs/audience` のインストールから。

```bash
cd node_modules/@slidxjs/audience
wrangler login   # あなたのもので、slidx のものではない
wrangler deploy
```

`wrangler deploy` が印字するオリジンは、
`https://slidx-audience.<account>.workers.dev` のようなものです。その文字列が
エンドポイントです。slidx がアカウントを聞くことはなく、toml はトークンを名指しません。

## 2. デッキを向ける

```ts
import { defineConfig } from "vite";
import { slidx } from "@slidxjs/vite-plugin";

export default defineConfig({
  plugins: [
    slidx({
      audience: {
        endpoint: "https://slidx-audience.example.workers.dev",
        room: "zero-js",
        // プレゼンターページだけ。省略すると話し手も参加者として入ります。
        hostKey: "a-key-you-chose",
      },
    }),
  ],
});
```

`room` は Durable Object の名前です。小文字、数字、ハイフン。URL スラッグと同じ規則です。
大文字は別のルームで、聴衆の半分が空の方に座ることになるので、Worker が拒む名前には
クライアントを注入しません。

`hostKey` は話し手のものです。プレゼンターページに書かれ、オーディエンススライドには
書かれません。アカウントではなく、slidx は他のどこにも保存しません。

`audience` を外すと、クライアントは出ません。デフォルトのデッキはそれでも何も取りません。

同じ Worker が [フォンリモート](remote.md) もホストします。第二の Durable Object クラスで、
第二のルート — `/sessions/<id>/socket` — なので、Q&A ルームとペアリングが秘密を共有する
ことはありません。`remote.endpoint` を同じオリジンへ向けてください。slidx はそれでも
セッションを持ちません。

## slidx がしないこと

Worker を代わりに動かしません。slidx が運用するリレーはこれをサービスにし、それは
slidx が何かという問いであり、非目標がすでに拒んでいます。運用者は著者の Cloudflare
アカウントです。
