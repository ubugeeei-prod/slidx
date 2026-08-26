---
title: はじめる
summary: 書いた Markdown、同じファイルを書くビジュアルエディタ、ネットワークなしで開ける静的 HTML。
section: start
order: 1
---

[English](/)

# Markdown を書く。ウェブサイトが届く。

slidx は Markdown のデッキを、普通の静的 HTML にコンパイルします。スライドごとに
一つの URL。クライアント側のルータはなく、起動すべきランタイムもありません。
ビジュアルエディタが書く先は **同じファイル** です。ビルドしたデッキは、自分自身
以外のどこにも何も求めません。

<video src="../../media/deck.webm" controls loop muted playsinline preload="metadata" width="960"></video>

ビルドしたデッキを、プレゼン用リモコンが送る矢印キーで進めています。このサイトの
写真と録画はすべて、実際の実行から `node scripts/record.mjs` が再生成します。
事実でなくなったものは再現に失敗するので、存在しない製品の写真が静かに残ることは
ありません。

## 六十秒

slidx は **まだ npm にも crates.io にもありません**。`vp add -D @slidxjs/vite-plugin`
は今日は何も入れません。リリースされたときの設定は、これだけです。

```bash
vp add -D @slidxjs/vite-plugin
```

```ts
// vite.config.ts
import { defineConfig } from "vite";
import { slidx } from "@slidxjs/vite-plugin";

export default defineConfig({ plugins: [slidx()] });
```

```bash
vp dev     # デッキと、/__slidx/ のビジュアルエディタ
vp build   # スライドごとに一つの HTML 文書
```

`slides/0001.md` を書けば、画面に出ます。

初回リリースまでは、そのインストールは未来の話です。
[クローンからビルドしたデッキまで](start.md) が、いま実際に動く道です。この
リポジトリ自身の CI が走らせているコマンドと同じものです。

## どの扉か

- **書きたい** → [ウォークスルー](start.md)、それから
  [フロントマター](frontmatter.md)、[レイアウト](layout.md)、[ステップ](steps.md)
- 見た目を決めたい → [組版](typography.md) と、デッキの `theme:`
- **発表したい** → [前夜](tonight.md)。症状ごとの索引です
- **配りたい** → [CLI](cli.md)（`export`、`publish`）
- 一枚に **フレームワークのコンポーネント** を置きたい → [islands](islands.md)
- 会場から **質問を受けたい** → [オーディエンスチャネル](audience.md)
- **電話** からデッキを進めたい → [フォンリモート](remote.md)
- 去年のデッキを **見つけたい** → [CLI](cli.md)（`list`、`grep`、`cd`）

## もう二人

- 数週間後に枠があり、まだ選んでいる →
  [トークに slidx を選ぶ](choosing.md)
- 明日発表で、何かがおかしい →
  [前夜](tonight.md)
