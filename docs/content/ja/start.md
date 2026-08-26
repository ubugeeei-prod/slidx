---
title: 何もないところからビルドしたデッキまで
summary: クローンから、書いて、lint して、発表して、ビルドしたデッキまで二十分。
section: start
order: 2
---

# デッキを作り、発表する

slidx が _何か_ は [はじめる](index.md) にあります。このページは、今日実際に動く道です。
npm にまだないのでクローンです。終わりには、自分で書いたデッキ、ノート PC では見えなかった
ものを捉えたリンタ、時計の入ったプレゼンタービュー、ネットワークなしで開く HTML の
ディレクトリがあります。

<video src="../../media/deck.webm" controls loop muted playsinline preload="metadata" width="960"></video>

ビルドしたデッキを、プレゼン用リモコンが送る矢印キーで進めています。このサイトの写真と
録画はすべて、実際の実行から `node scripts/record.mjs` が再生成します。事実でなくなった
ものは再現に失敗するので、存在しない製品の写真が静かに残ることはありません。

このサイトをもう二人が読みます。その一人なら、先にそちらへ。

- 数週間後に枠があり、まだ選んでいる →
  [トークに slidx を選ぶ](choosing.md)
- 明日発表で、何かがおかしい →
  [前夜](tonight.md)

## 二十分を使う前に

slidx は **まだ npm にも crates.io にもありません**。`npm i @slidxjs/vite-plugin` は
今日は何も入れません。初回リリースまではクローンから動かすのがこのページのやり方で、
ここにあるコマンドはこのリポジトリ自身の CI が走らせているものです。動くし、それを
置き換える二行のインストールではありません。

先に知っておく価値があります。デッキの形式はトークを書くのに足りるだけ固まっています。
インストールの話はまったく固まっていません。

## 1. ツールチェーン

Node と pnpm、それに Rust。パーサ、ステップコンパイラ、リンタ、ハイライタ、レンダラは
Rust で、一つの WebAssembly モジュール経由で届き、そのモジュールはコミットではなく
ビルドされます。

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
```

`wasm-bindgen-cli` は `crates/slidx_wasm/Cargo.toml` がピンしている版と一致しなければ
なりません。ビルドは両方を読み、後で生成された糊の中で再コンパイルの話になる前に、
二つの数字で止まります。

```bash
git clone https://github.com/ubugeeei-prod/slidx
cd slidx
vp install
vp run build:packages
```

[Vite+](https://voidzero.dev) がこのリポジトリのすべてのタスクを回します。Rust も
TypeScript も。`build:packages` が遅い方で、パイプラインを WebAssembly にコンパイルします。
一度だけ走らせます。

## 2. 書く前にデッキを見る

```bash
vp exec --filter slidx-example-deck -- vite dev
```

Vite が印字するポートの **`/slides/`** を開く。それが 1 枚目です。`/slides/2/` が
2 枚目で、ルートではなく URL なので、共有、ブックマーク、電話で開く、索引ができます。
同じデッキに、あと四つ住所があります。

| URL                  | 何か                                                       |
| -------------------- | ---------------------------------------------------------- |
| `/slides/`           | デッキ。1 枚目から                                         |
| `/slides/2/`         | 2 枚目。それ自体                                           |
| `/slides/presenter/` | プレゼンタービュー。時計、ノート、次のスライド             |
| `/slides/remote/`    | フォンリモート。デッキが Worker を名指したとき             |
| `/slides/print/`     | デッキ全体を一つの文書。アニメーションの停留ごとに一ページ |
| `/__slidx/`          | ビジュアルエディタ。dev のみ                               |

`/__slidx/` を開いてスライドをクリックする。エディタは、自分のエディタで開いている
Markdown ファイルに書きます。編集は保存したファイルへのバイト範囲の splice なので、
空行と `*` の箇条書きはキャンバスから編集しても残ります。

<video src="../../media/editor-tour.webm" controls loop muted playsinline preload="metadata" width="960"></video>

これは一続きのエディタセッションです。インラインのテキストとアドレスしたスタイル、
ブロックの色、八つのハンドルのリサイズ、自由移動と整列ガイド、画像と動画のドロップ、
Markdown の `<style>` に書き戻されるレイアウト、トランジション、スライドの作成、
複製と並べ替えのショートカット、undo と redo、同じ操作を受けて送る第二のエディタ。
`vp run record:tour` がセッション全体をもう一度演じます。

## 3. スライドを書く

サンプルデッキのスライドは `examples/deck/slides` にあり、一ファイル一枚です。
五枚目、`0005.md` を足します。

```md
---
budget: 60s
---

## What I actually measured

- Build time fell to 28ms <!-- step -->
- The PDF stopped losing the animation <!-- step -->
```

保存する。ブラウザはもう更新しています。

そのファイルの二つのものが Markdown ではなく slidx です。`budget: 60s` はこの
スライドにかけるつもり時間で、プレゼンタービューが遅れていると言える根拠です。
各 `<!-- step -->` はアニメーションの停留です。箇条書きは一つずつ現れ、同じ
タイムラインがプロジェクタ、プレゼンタービュー、印刷ページを動かすので、書いた
アニメーションが印刷されるアニメーションです。

## 4. わざと壊す

ノート PC からは見えにくい部分です。インターネット上のロゴをスライドに置き、
箇条書きを九つにします。

```md
![our logo](https://example.com/logo.png)

- one
- two
- three
- four
- five
- six
- seven
- eight
- nine
```

それからビルドします。

```bash
vp exec --filter slidx-example-deck -- vite build
```

ビルドは止まります。同じ規則が同じスライドを読み、`vite build` からでもバイナリから
でも同じです。これが `slidx lint` の言い方です。実際の実行を、ここの写真を再生成する
スクリプトが撮っています。

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../../media/terminal-lint-dark.png">
  <img alt="リンタが、ビルドを止めるオフラインエラーと、助言としての箇条書き数を報告している" src="../../media/terminal-lint-light.png">
</picture>

リモート画像は **エラー** で、ビルドを止めます。オフライン保証が推奨ではなく強制されて
いるということです。何かを取るデッキは、Wi-Fi の動かない部屋で失敗するデッキで、それを
知る信頼できる時点はいまだけです。九つの箇条書きは **info** です。12 列目から読めない
スライドについての観察であり、slidx が決めたことではありません。

診断はどれもコード、位置、次にすることを持ちます。手を打てない警告はノイズです。

リモート画像を戻して、もう一度ビルドします。

## 5. 発表する

dev サーバを動かしたまま、第二のウィンドウで `/slides/presenter/` を開きます。

時計がページで一番大きいものです。デッキがフロントマターで宣言した枠 —
`duration: 20m` — に対して数え、切れるときではなくその前に知らせます。その下の
遅れ／余裕は、書いたスライドごとの予算と比べ、超過しているときは `optional: true`
と印したスライドを名指すので、どれを落とすか分かります。

矢印キーはプレゼンターウィンドウからデッキを進めます。クリッカーが送る先がそこだから
です。オーディエンスウィンドウはブロードキャストチャネルで従います。それが使えない
ところではミラーリングはオフで、デッキはそれでも発表できます。

## 6. ビルドして、ケーブルを抜く

```bash
vp exec --filter slidx-example-deck -- vite build
```

```
examples/deck/dist/slides/
├── index.html            slide 1
├── 2/index.html          slide 2
├── 2/presenter/index.html
├── print/index.html      the whole deck, one page per stop
├── og-2.png              a social card per slide
└── runtime.js
```

ネットワークを切った状態で、ファイルシステムから `dist/slides/index.html` を開きます。
描画されます。サーバもルータもフレームワークもありません。停留のあるスライドは共有
モジュールと自分のコンパイル済みタイムラインを一つ読み、停留のないスライドは何も
読みません。

## いま手元にあるもの

リポジトリの Markdown であり、`dist` の静的 HTML であるデッキ。ネットワークに手を
伸ばすスライドの出荷を拒むビルド。残り時間を知っているプレゼンタービュー。そして
アニメーションの停留がすべて入った印刷文書。配布物が、したトークと別の話にはなりません。

## 次はどこへ

- バイナリは別で任意であり、ビルドができないことをします。最初は `slidx doctor` で、
  これから話すマシンを調べます。`cargo build --release -p slidx_cli` でビルドし、
  `./target/release/slidx doctor`。
- [トークに slidx を選ぶ](choosing.md) は、何が作られ、何がなく、何が今後もなされないかの
  正直な説明です。
- [前夜](tonight.md) は概念ではなく症状の索引で、いまブックマークして十一時に開く
  ページです。
- [フレームワーク islands](islands.md) は Vue、React、Svelte、Solid、Angular の
  コンポーネントのためのオプトインで、他のスライドは静的なままです。
- [ROADMAP.md](../../../ROADMAP.md) に、終わっていないものが住んでいます。チェックされた
  箱が何を意味してよいかを最初に定義していて、このプロジェクトが苦労して学んだ区別です。
