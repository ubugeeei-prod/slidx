# Releasing

Releases are cut by pushing a tag. Everything after that is CI, authenticating
to both registries with GitHub OIDC — **no publish token is stored in this
repository, and none should ever be added**.

```bash
vp run release minor
```

```bash
vp run release --tag
```

Two commands, because `main` takes pull requests and requires `ci`. The first
writes the version into every place one lives — the Cargo workspace, the version
each crate is required at by its siblings, the lockfile, every publishable
`package.json`, and generated brand assets — commits it to a `release/vX.Y.Z`
branch, and opens the pull request with auto-merge armed. `--dry-run` writes the
tree and stops before the commit.

Merging that publishes nothing. The second command, run afterwards, reads the
version off `origin/main` and tags the commit that merged — and **that** is what
starts the release. It reads the merged version rather than the working tree's
on purpose: those are different questions, and tagging the second is how a tag
comes to name bytes nobody agreed to publish.

It refuses for the reasons a release goes wrong before it starts: a dirty tree,
a branch that is not `main`, a `HEAD` that is not `origin/main`, and a tag that
already exists — here, or on the remote. Then it runs `check:version` against
the tag it is about to create — the same check the release workflow runs before
it publishes — so a version living somewhere this script does not know about
stops the release here rather than halfway through a registry.

It used to be one command, ending in `git push origin main`, and the branch
rules refused that push on every version. Nobody found out until the first
release, because its tests read the files it wrote and never the push it ended
with. It also failed _after_ committing and tagging locally, so each attempt
left a half-applied release and a tag that made the next attempt refuse — which
is why nothing is tagged until the version is on `main`, and why the leftover
tag is now named in the error rather than left to be guessed at.

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
`@slidxjs/cli-darwin-arm64` and friends — each holding a single binary and
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

Before any of it, one thing has to exist that no command here creates: **an npm
organisation named `slidxjs`**. A scope is either your username or an
organisation, and the username publishing this is `ubugeeei` — so `@slidxjs/*`
has nowhere to go until the organisation does. It is free for public packages,
and it is created at `https://www.npmjs.com/org/create`. `npm publish` fails
with a 404 on the scope rather than saying so, which is a confusing place to
learn this.

The artifacts to publish can all be built by CI rather than by hand: run the
**Registry bootstrap artifacts** workflow on `main` and download its bundle. It
cross-compiles every platform binary on a native runner and packs every npm
tarball in publish order, with `publish-order.txt` beside them. It has
`contents: read` and publishes nothing — the commands below are then `npm
publish <tarball>` against files that already exist, rather than a build per
platform on a laptop.

### crates.io

```bash
cargo login
```

```bash
node scripts/publish-crates.mjs
```

The publisher stops on the first real failure, waits for the exact retry time
when crates.io rate-limits a new name, and skips versions that already exist.
That makes the command safe to rerun after an interruption. Without stopping,
one crate that cannot publish becomes a dozen that cannot resolve it, and the
error you are left reading is the last one — `no matching package named
slidx_dialect found`, from a crate whose real problem was six crates earlier.

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
pnpm exec vp run build:packages
```

```bash
bootstrap_dir=$(mktemp -d)
node scripts/pack-npm.mjs \
  "$bootstrap_dir" \
  $(node scripts/publish-order.mjs npm) > "$bootstrap_dir/source-tarballs"
node scripts/publish-npm.mjs --list "$bootstrap_dir/source-tarballs"
```

`pack-npm.mjs` is the same path the release workflow uses. It resolves every
`workspace:*` requirement to this release's exact version and refuses a tarball
if one survives. Publishing the workspace directories directly skips that
rewrite and would put an unusable dependency requirement on the registry.

The order is derived the same way and for the same reason. The list here used to name
`@slidxjs/wasm` and `@slidxjs/runtime` only, and `release.yml` agreed with it —
which left **`@slidxjs/vite-plugin`**, the package the README tells people to
install, unpublished.

The `slidx` wrapper is not in that list. Its dependencies are the five
`@slidxjs/cli-*` packages, which do not exist until the release builds them, so
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
node scripts/pack-npm.mjs \
  "$bootstrap_dir" \
  packages/cli/dist/* \
  packages/cli > "$bootstrap_dir/cli-tarballs"
node scripts/publish-npm.mjs --list "$bootstrap_dir/cli-tarballs"
```

Then, for **each** package — everything `publish-order.mjs npm` lists, plus
`slidx` and the five `@slidxjs/cli-*` — at
`https://www.npmjs.com/package/<name>/access`, add a trusted publisher with the
same repository and `release.yml`.

`slidx` is an unscoped name and `@slidxjs` is a scope: publishing `slidx` and
`@slidxjs/runtime` for the first time claims both package names, so do that
before anyone else does.

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
