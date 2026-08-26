---
title: フレームワーク islands
summary: Vue、React、Svelte、Solid、Angular のコンポーネントを、デッキ全体をその上で動かさずに一枚へ足します。
section: reference
order: 6
---

# フレームワーク islands

スライドはまず Markdown と完結した HTML です。一部分が本当にコンポーネントを要する場合、
その要素を island と印し、使うコンポーネントランタイムだけを登録します。デッキの残りは
静的です。island のセットアップなしではクライアントエントリは出ず、印した要素のない
ページは、他のスライドがあってもそのエントリを読みません。

## 1. デッキをセットアップへ向ける

セットアップモジュールは Vite ルートからの相対です。

```ts
import { defineConfig } from "vite";
import { slidx } from "@slidxjs/vite-plugin";

export default defineConfig({
  plugins: [
    slidx({
      islands: "./islands.ts",
      mdx: true,
    }),
  ],
});
```

`islands` がクライアントランタイムのオプトインです。`mdx` は別です。既定のスライド拡張子に
`.mdx` を足し、コンポーネント構文を有効にします。両方外せば普通の `.md` の道はそのままで、
オーディエンススライドは JavaScript ゼロです。

## 2. デッキが使うものだけ登録する

この例は Vue を選びます。コンポーネントと Vue 自身は、island が見えたときに読み、
1 枚目が開いたときには読みません。

```ts
import { createRegistry } from "@slidxjs/islands";
import { vueIsland } from "@slidxjs/islands/vue";

export default createRegistry([
  vueIsland({
    name: "Counter",
    component: () => import("./components/Counter.vue"),
  }),
]);
```

フレームワークはデッキ全体が入るモードではなく、コンポーネントごとのアダプタです。デッキは
その Vue カウンタの隣に React のチャートを登録でき、どちらかの登録を外すとそのアダプタと
コンポーネントが Vite のグラフから消えます。

| 選択    | アダプタの入口             | ファクトリ      | デッキへのインストール                 |
| ------- | -------------------------- | --------------- | -------------------------------------- |
| Vue     | `@slidxjs/islands/vue`     | `vueIsland`     | `vue` と Vue の Vite プラグイン        |
| React   | `@slidxjs/islands/react`   | `reactIsland`   | `react`、`react-dom`、React プラグイン |
| Svelte  | `@slidxjs/islands/svelte`  | `svelteIsland`  | `svelte` と Svelte の Vite プラグイン  |
| Solid   | `@slidxjs/islands/solid`   | `solidIsland`   | `solid-js` と Solid の Vite プラグイン |
| Angular | `@slidxjs/islands/angular` | `angularIsland` | Angular 20+ とそのコンパイラプラグイン |

Angular のコンポーネントと公開パッケージは、デッキの Vite 設定に Angular 自身のコンパイラと
リンカが要ります。アダプタは zoneless で動くので、一つの island がすべてのスライドの
タイマ、Promise、イベントリスナをパッチしません。

## 3. Markdown に完結したフォールバックを置く

`mdx: true` では、大文字のタグが同じ名前のレジストリエントリを選びます。

```mdx
## Sign-ups

<Counter start={128} label="people">

**128 people**

</Counter>
```

文字列属性と、波括弧の中の JSON 値が props になります。配列とオブジェクトも許されます。
import は不要です。セットアップのレジストリが `Counter` を解決するので、その一つの登録を
外すとそのフレームワークとコンポーネントも Vite のグラフから消えます。

コンパイラはデッキからの式を実行しません。`start={window.total}` のような値は、止める
`mdx/non-static-props` 診断になり、island マーカーなしでフォールバックを描画し、ビルド中に
走れません。

`.mdx` ファイルはエディタの真実の源のままです。見た目のテキスト、スタイル、レイアウト、
アニメーション、スライド順、undo、共有編集はそのファイルを splice します。MDX は描画のため
だけにコンパイルされます。コードフェンスと小文字の HTML はそのままです。

明示形は普通の `.md` でも動きます。island 名がレジストリエントリを選び、props は一つの
JSON 属性を渡ります。

```md
## Sign-ups

<div
  data-slidx-island="Counter"
  data-slidx-island-props='{"start": 128, "label": "people"}'
>
  <strong>128 people</strong>
</div>
```

子は読み込みの飾りではありません。静的な答えです。ソーシャルカード、印刷／PDF、失敗した
コンポーネント import、JavaScript を切って開いたスライドは、どれもそれを見せます。
hydrator はマウントが失敗したら同じマークアップを戻すので、一つのコンポーネントが
ステージ上でスライドを空白にできません。

props は実行可能な式ではなく JSON です。壊れた JSON は報告され、コンポーネントは空の
オブジェクトでマウントし、ページの残りを倒す代わりにフォールバックを残します。

## ライフサイクル

island はページの読み込み中に解決され、見えたときだけマウントされ、離れたらアンマウント
されます。各アダプタは自分の持つフレームワークオブジェクトを解放します。Vue の app、
React の root、Svelte の instance、Solid の owner、Angular の application と component。
スライドへ戻ると、最初の上に積み重ねるのではなく、新しいマウントが一つできます。

プレゼンタービューの次スライドのプレビューは静的なままです。コンポーネントを一枚早く
始めず、印刷文書はクライアントを import しません。
