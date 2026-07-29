# Releasing

Releases are cut by pushing a tag. Everything after that is CI, authenticating
to both registries with GitHub OIDC — **no publish token is stored in this
repository, and none should ever be added**.

```bash
git tag v0.1.0 && git push origin v0.1.0
```

The tag must match `version` in `[workspace.package]`. CI checks it before it
publishes anything, because a mismatch is only visible once it is permanent on
a registry.

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
for crate in slidx_core slidx_lint slidx_theme slidx_render slidx_doctor slidx_cli; do cargo publish -p "$crate"; done
```

`slidx_cli` is published to crates.io too, so `cargo install slidx_cli` works on
a platform with no prebuilt binary. It depends on the four above, so it goes
last.

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
cd packages/runtime && npm publish --access public
```

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

Then, for **each** package — `@slidx/wasm`, `@slidx/runtime`, `slidx`, and the
five `@slidx/cli-*` — at `https://www.npmjs.com/package/<name>/access`, add a
trusted publisher with the same repository and `release.yml`.

`slidx` is an unscoped name and `@slidx` is a scope: publishing `slidx` and
`@slidx/runtime` for the first time claims both, so do that before anyone else
does.

### Checking it worked

The next tag push should publish with no token anywhere. If npm silently falls
back to a token, the usual cause is `NODE_AUTH_TOKEN` being set in the
environment — npm prefers it and skips OIDC without saying so.

## Versioning

One version across the whole workspace, Rust and TypeScript alike. Bump
`[workspace.package]` in `Cargo.toml` and every publishable `package.json`
together; `node scripts/check-version.mjs` is what stops them drifting and runs
as part of `vp check`.

Pre-1.0, a breaking change is a patch bump. Say what broke in the release
notes.
