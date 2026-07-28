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
3. Builds and publishes the npm packages with provenance.
4. Opens a GitHub release with generated notes.

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
for crate in slidx_core slidx_lint slidx_theme slidx_render; do cargo publish -p "$crate"; done
```

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

Then, for **each** package, at
`https://www.npmjs.com/package/<name>/access`, add a trusted publisher with the
same repository and `release.yml`.

`@slidx` is an npm scope: publishing `@slidx/runtime` for the first time also
claims it, so do that before anyone else does.

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
