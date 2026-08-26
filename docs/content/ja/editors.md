---
title: エディタ
summary: 診断、補完、デッキのアウトライン、ホバー、整形。VS Code、Zed、Neovim で。
section: reference
order: 8
---

# エディタ

slidx には言語サーバがあります。入力中に、次をくれます。

- **診断** — `slidx lint` が走らせるすべての規則と、付いた手当て。プロジェクタが白飛ばす
  コントラストの対、12 列目が読めない本文、alt のない画像、そこにないマークを名指す
  `steps:` の項目。
- **補完** — フロントマターのキー、テーマ名、トランジション、ステッププリセット、
  レイアウト、アスペクト比。どれも定義している Rust から読むので、slidx に足したプリセットは
  エディタプラグインを誰も編集せずに出ます。
- **デッキのアウトライン** — スライドごとに一つ。停留、予算、任意と印したか。下に
  ネストしたステップ。
- **ホバー** — フロントマターのキーが期待するものと、プリセットが画面で実際にすること。
  コンポジタに留まるかも含みます。
- **整形** — 保存時の `slidx fmt`。ファイルの書き直しではなく、小さな編集の束。散文、
  折り返し、箇条書きの印は触れません。

`slidx lsp` で、一つのバイナリのサブコマンドです。入れる `slidx-lsp` はありません。
一つのバイナリは PATH 上の一つのもの、一つのリリース資産、プロジェクトの
`.slidx-version` ピンが適用される一つの版です。

## 対象のファイル

**`slides/` ディレクトリの下の Markdown。** それ以外はありません。

Vite プラグインが既定でビルドする配置であり、`slidx lint`、`slidx fmt`、`slidx dev` が
どれも落ちる道です。正直な、いちばん狭い規則でもあります。デッキは Markdown で、たいていの
Markdown はデッキではなく、一つの `talk.md` と README を見分ける唯一の方法は、持っている
Markdown を全部開いて読むことです。changelog にスライドの診断を置いたエディタプラグインは、
外される資格があります。

何も起きない理由を不思議がる前に、知っておくべき帰結が二つあります。

プロジェクトのてっぺんの **一つのファイル** として置いたデッキは拾われません。`slides/` の
下へ移してください。ビルドするプラグインにとっても、それがデッキになるものです。

プラグインの `srcDir` を別の場所へ向けたプロジェクトも拾われません。サーバはファイルパスを
渡され、属する Vite 設定については何も知らず、ディレクトリ名からの推測は同じ越権です。

規則はプラグインではなく **サーバ** にあります。Zed は言語サーバを言語全体にしか付けられず、
クライアントはパス規則をまったく表現できません。各クライアントに述べた規則は、二つが守り
一つが守れない規則です。フィルタできるエディタはします。ファイルを送らずに済むからです。
ただし、彼らがすることには何も依りません。

## シンタックスハイライトは、意図的にない

これらのプラグインはどれも、言語、文法、ファイル関連を出しません。デッキは Markdown の
ファイルのままで、Markdown の道具が乗っています。プレビュー、表の整形、折りたたみ、
treesitter。

代わりはハイライタを持つ `slidx` 言語で、古くなります。補完がプリセット、トランジション、
テーマ、レイアウトを知っているのは、定義している Rust を読むからです。TextMate 文法や
Vim の syntax ファイルは Rust を読めないので、それらのリストはどれも二度目に打ち出され、
誰かが変種を足した最初のときに間違います。静かに、気づくテストはどこにもありません。

方言自身の構文は、ハイライタにとってすでに普通の Markdown です。`<!-- step: fade -->` は
コメント、`[3.2x faster]{#result}` はリンクのようなスパン、`---` は罫線です。それらが
であるものとして色が付きます。

## バイナリを見つける

ここのプラグインはどれも、同じ順で見ます。

1. **設定したもの。** 何か設定したなら。言われた通りに取ります。静かに別のものへ落ちる
   設定は、誰かが一時間間違ったバイナリをデバッグするやり方です。
2. **PATH 上の `slidx`。** 自分のターミナルが走らせるもので、したがって
   `slidx version use` とプロジェクトの `.slidx-version` が作用するものです。
3. **入れたディレクトリ** — `$SLIDX_HOME`、なければ `$XDG_DATA_HOME/slidx`、なければ
   `~/.slidx`、Windows では `%LOCALAPPDATA%\slidx`。`install.sh` が書く順と
   `slidx version` が管理する順と同じです。

3 があるのは、エディタがログインシェルではないからです。ドックから起動したアプリケーションは、
セッションマネージャが与えた PATH を持ち、macOS ではたいていプロファイルのそれではありません。
Zed はこれを飛ばし、正しく飛ばします。すでにプロジェクトのシェル環境経由で解決するので、
答えはターミナルの答えです。

**何も見つからないとき**、プラグインは一度そう言い、見た場所と直し方を名指します。サーバを
始めず、静かに失敗しません。始まらない言語サーバは、言うことのない言語サーバと区別できず、
そう見えてはいけない唯一のものです。マシンが何が起きていると思っているかを知るには。

```bash
slidx version current
```

実際に走っているファイル、どの入れ方がそこに置いたか、PATH 上の他のものがそれを隠して
いるかを印字します。

## VS Code

拡張は [packages/vscode](../../../packages/vscode) にあります。Marketplace にはまだありません。
slidx にリリースがないので、今日はビルドから入れます。

```bash
vp run build:vscode
npx @vscode/vsce package --out slidx.vsix
code --install-extension slidx.vsix
```

ワークスペースに `slides/*.md` があるときだけ起き、それより前には起きません。デッキの
ないウィンドウは何も始めません。

設定は一つ。誰も代わりに推測できない一つのことです。

```json
{ "slidx.path": "/opt/built/slidx" }
```

空なら、上の順が適用されます。

すべての Markdown の整形器にせず、デッキの整形器にするには。

```json
{
  "[markdown]": { "editor.formatOnSave": true },
  "editor.defaultFormatter": "esbenp.prettier-vscode"
}
```

VS Code は二つが手を挙げた最初のときにどれを使うか聞き、slidx が手を挙げるのは
`slides/` の下のファイルだけです。

## Zed

拡張は [editors/zed](../../../editors/zed) にあります。Zed の拡張は Zed 自身がコンパイル
するので、先にビルドするものはありません。

1. **Extensions → Install Dev Extension**
2. クローンの `editors/zed` を選ぶ。

Markdown に付きます。Zed が出せるいちばん細かい粒度で、サーバはデッキでないものをすべて
拒みます。

特定のバイナリへ向けるには、Zed の設定で。

```json
{ "lsp": { "slidx": { "binary": { "path": "/opt/built/slidx" } } } }
```

## Neovim

Neovim 用の slidx プラグインはなく、あるべきでもありません。Neovim 0.11 は runtimepath
から `lsp/<name>.lua` を読み、`nvim-lspconfig` はまさにそれらのファイルのディレクトリなので、
ここでのプラグインは自分で書ける表のラッパになります。入れるもの、版を合わせるものが
もう一つ増え、何も買いません。

表は [editors/nvim/lsp/slidx.lua](../../../editors/nvim/lsp/slidx.lua) で、ここに読むのに
短いです。

```lua
return {
  cmd = { "slidx", "lsp" },
  filetypes = { "markdown" },
  root_markers = { "slides", ".slidx-version", "vite.config.ts", "vite.config.js", ".git" },
}
```

`~/.config/nvim/lsp/slidx.lua` へコピーして、点けます。

```lua
vim.lsp.enable("slidx")
```

またはコピーせずクローンを runtimepath に置き、更新の道がある同じことです。

```lua
vim.opt.runtimepath:append("/path/to/slidx/editors/nvim")
vim.lsp.enable("slidx")
```

**新しい filetype はありません。** デッキは `markdown` バッファで、そう留まります。上の
理由です。`slidx` filetype は、すでに用意したすべての Markdown のものを失わせます。
クライアントは Markdown に付き、サーバがどのバッファがデッキかを決めます。

README にまったく付いてほしくないなら、`vim.lsp.enable` の代わりに autocommand を使います。
通信を省き、答えは変わりません。

```lua
vim.api.nvim_create_autocmd({ "BufReadPost", "BufNewFile" }, {
  pattern = "*/slides/*.md",
  callback = function(event)
    vim.lsp.start(vim.lsp.config.slidx, { bufnr = event.buf })
  end,
})
```

保存時の整形。デッキだけ。

```lua
vim.api.nvim_create_autocmd("BufWritePre", {
  pattern = "*/slides/*.md",
  callback = function() vim.lsp.buf.format({ name = "slidx" }) end,
})
```

## 何かではないこと

何かの第二の実装ではありません。診断はどれも `slidx lint` が印字するコードと手当てを運び、
整形の編集はどれも `slidx fmt` が書いたはずの splice で、補完リストはどれも、補完している
ものを定義している Rust から読み出されます。`slidx lint` と食い違うものを見せるエディタは
意見の違いではなくバグです。

デッキが _どう見えるか_ については何も言いません。十五列目から活字が読めるか、色の対が
プロジェクタを生きるかは、描画されたスライドについての問いで、リンタが答え、ここで走ります。
ただしエディタのペインの中のものは、会場についての証拠ではありません。それを決めるのは
いまでも `slidx lint` と実ブラウザです。
