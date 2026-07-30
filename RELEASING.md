# Releasing

Releases are cut by pushing a tag. Everything after that is CI, authenticating
to both registries with GitHub OIDC — **no publish token is stored in this
repository, and none should ever be added**.

```bash
vp run release minor
```

`major`, `minor` or `patch`. It writes the version into every place one lives —
the Cargo workspace, the version each crate is required at by its siblings, the
lockfile, and every publishable `package.json` — commits that, and pushes the
tag. `--dry-run` writes the tree and stops before the commit.

It refuses for the four reasons a release goes wrong before it starts: a dirty
tree, a branch that is not `main`, a tag that already exists, and a `HEAD` that
is not `origin/main`. Then it runs `check:version` against the tag it is about
to create — the same check the release workflow runs before it publishes — so a
version living somewhere this script does not know about stops the release
here rather than halfway through a registry.

The tag must match `version` in `[workspace.package]`; that is what
`check:version` is asserting. A mismatch is otherwise only visible once it is
permanent on a registry.

## What CI does

1. Runs the full check graph again. A tag is not a promise that the tree is
   good, and a bad publish cannot be taken back.
2. Mints an OIDC token, exchanges it for a short-lived registry credential, and
   publishes the crates **in dependency order** — crates.io has to see each
   crate before the one that depends on it can resolve.
3. Cross-compiles `slidx` for every platform in
   [`scripts/platforms.mjs`](./scripts/platforms.mjs), on a native runner
   wherever GitHub has one.
4. Builds and publishes the npm packages with provenance, again in dependency
   order: the per-platform binary packages first, the `slidx` wrapper last. A
   wrapper on the registry before its optional dependencies installs without a
   binary, and npm calls that a success.
5. Opens a GitHub release carrying the archives, `SHA256SUMS`, `install.sh`,
   and a build attestation for each binary.

## The binary, and the two ways to get it

`slidx` is one prebuilt executable per platform. Both install channels hand
over the same file; neither compiles anything and neither needs Node.

### `npm i -g slidx`

The wrapper package `slidx` declares one `optionalDependency` per platform —
`@slidx/cli-darwin-arm64` and friends — each holding a single binary and
declaring the `os` and `cpu` it runs on. npm installs the one that matches and
skips the rest, and `packages/cli/bin/slidx.mjs` execs it.

**There is no `postinstall`, and there must never be one.** A package that
downloads its executable at install time breaks offline installs, breaks cached
CI, breaks behind a corporate proxy, and cannot be audited from its published
contents — which is indistinguishable from an attack whatever the intent. The
binary being _in_ the tarball is what makes `npm ci --offline` work, puts an
integrity hash in the lockfile, and lets `--provenance` attest the executable
rather than a script that will later fetch one.

The platform packages are generated at release time by
`scripts/build-platform-packages.mjs` rather than checked in, so their version,
their contents and the platform list have nowhere to drift from.

### `curl … | sh`

[`install.sh`](./install.sh) detects the platform, downloads the matching
archive and `SHA256SUMS` from the same release, verifies one against the other,
and installs to `~/.slidx/bin` — the directory the version manager will own, so
the two never end up managing different binaries.

**The checksum is not optional.** A missing checksum file, an asset the file
does not mention, or a mismatch all stop the install and delete the download.
Be straight about what that buys: both files come from the same server, so it
proves the download arrived intact and unswapped, not that the account was not
compromised. For that, the release also publishes a Sigstore attestation:

```bash
gh attestation verify slidx-aarch64-apple-darwin.tar.gz --repo ubugeeei-prod/slidx
```

### `slidx version`

A third way to get a binary, and the only one that keeps several. It reads the
same release assets and the same `SHA256SUMS` as `install.sh` — one
publication, two readers — so a version installed either way is the same file.

It verifies with its own SHA-256 rather than looking for `sha256sum` on the
machine. That is the one place the binary is strictly better than the shell
script: there is no detection, no fallback, and no branch that installs without
checking.

Nothing extra has to be published for it. If a release has archives and a
checksum file, `slidx version install` can install from it.

### Adding a platform

Add it to `scripts/platforms.mjs` and to the `binaries` matrix in
`release.yml`. Nothing else — the wrapper's dependencies, the installer's table
and the workflow are held together by tests in `packages/cli/test`, which fail
naming whichever one you missed.

## One-time setup

Both registries work the same way: **the first version has to be published by a
human**, because trusted publishing is configured per package and there is
nothing to configure until the package exists. After that, no human touches a
registry again.

### crates.io

```bash
cargo login
```

```bash
for crate in $(node scripts/publish-order.mjs crates); do cargo publish -p "$crate"; done
```

The order is derived from the manifests rather than written down, because a
written-down order goes stale silently. It did: `slidx_cli` gained
`slidx_highlight` and `slidx_publish` as dependencies and neither was in the
list here or in `release.yml`, so a tag push would have published five crates
and then failed on the sixth — with the five already permanent.

`slidx_cli` is published to crates.io too, so `cargo install slidx_cli` works
on a platform with no prebuilt binary. It depends on six other crates, so it
goes near the end.

Then, for **each** crate, at `https://crates.io/crates/<name>/settings`, add a
trusted publisher:

| Field             | Value           |
| ----------------- | --------------- |
| Repository owner  | `ubugeeei-prod` |
| Repository name   | `slidx`         |
| Workflow filename | `release.yml`   |
| Environment       | leave empty     |

### npm

```bash
npm login
```

```bash
pnpm -r --filter "./packages/**" run pack:lib
```

```bash
for dir in $(node scripts/publish-order.mjs npm); do (cd "$dir" && npm publish --access public); done
```

Derived the same way and for the same reason. The list here used to name
`@slidx/wasm` and `@slidx/runtime` only, and `release.yml` agreed with it —
which left **`@slidx/vite-plugin`**, the package the README tells people to
install, unpublished.

The `slidx` wrapper is not in that list. Its dependencies are the five
`@slidx/cli-*` packages, which do not exist until the release builds them, so
it cannot be ordered from a manifest and the workflow places it last by hand.

The platform packages have to exist before they can be configured too, and they
need binaries, so the first release of those is a human running the build once
per platform:

```bash
cargo build --release -p slidx_cli --bin slidx --target <triple>
```

```bash
node scripts/build-platform-packages.mjs <binaries-dir>
```

```bash
for dir in packages/cli/dist/* packages/cli; do (cd "$dir" && npm publish --access public); done
```

Then, for **each** package — everything `publish-order.mjs npm` lists, plus
`slidx` and the five `@slidx/cli-*` — at
`https://www.npmjs.com/package/<name>/access`, add a trusted publisher with the
same repository and `release.yml`.

`slidx` is an unscoped name and `@slidx` is a scope: publishing `slidx` and
`@slidx/runtime` for the first time claims both, so do that before anyone else
does.

### Checking it worked

The next tag push should publish with no token anywhere. If npm silently falls
back to a token, the usual cause is `NODE_AUTH_TOKEN` being set in the
environment — npm prefers it and skips OIDC without saying so.

## Versioning

One version across the whole workspace, Rust and TypeScript alike. The number
appears in four kinds of place, and a bump has to reach all of them:

- `[workspace.package]` in `Cargo.toml`
- every `slidx_*` entry in `[workspace.dependencies]`, which states the version
  each crate is required at by its siblings; cargo will not publish a path
  dependency without one
- every publishable `package.json`, including the `slidx` wrapper's
  `optionalDependencies` on the platform packages the release builds
- `Cargo.lock`, via `cargo update --workspace`

Nothing here has to be remembered. `node scripts/check-version.mjs` fails on any
of them drifting except the wrapper's optional dependencies, which
`packages/cli/test/platforms.test.mjs` holds to the wrapper's own version, and
both run in CI as part of `vp check`.

Pre-1.0, a breaking change is a patch bump. Say what broke in the release
notes.
