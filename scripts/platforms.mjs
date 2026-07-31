/**
 * Every platform slidx ships a prebuilt binary for.
 *
 * One table, because this list is otherwise repeated in five places that can
 * silently disagree: the release matrix, the npm wrapper's optional
 * dependencies, the per-platform packages, the shell installer, and the
 * documentation. A platform present in four of them and missing from the fifth
 * is a release that publishes a package nobody can install, or an installer
 * that downloads a file nobody built — and neither shows up until somebody
 * tries it.
 *
 * The workflow matrix cannot import this file, because YAML has no imports. So
 * that one copy is checked against this table by a test rather than generated
 * from it, which catches the drift at the same moment either way.
 *
 * ## Why Linux is musl
 *
 * Both Linux targets are statically linked against musl rather than dynamically
 * against glibc. A glibc binary carries the version of whatever machine built
 * it, and running it on anything older fails with a message about
 * `GLIBC_2.34` that says nothing about slidx. A static musl binary runs on
 * every distribution including Alpine, which is what half of CI is, and the
 * malloc it gives up is irrelevant to a process that lives for two seconds.
 */

/**
 * @typedef {object} Platform
 * @property {string} npm      Package name, without the scope's `@slidxjs/` prefix stripped.
 * @property {string} target   Rust target triple. Also the name in the release asset.
 * @property {string} os       `process.platform`, and npm's `os` field.
 * @property {string} cpu      `process.arch`, and npm's `cpu` field.
 * @property {string} runner   The GitHub runner that builds it.
 * @property {boolean} windows Whether the binary is `slidx.exe` in a zip.
 * @property {string} describes What to call it when a person has to read it.
 */

/** @type {Platform[]} */
export const PLATFORMS = [
  {
    npm: "@slidxjs/cli-darwin-arm64",
    target: "aarch64-apple-darwin",
    os: "darwin",
    cpu: "arm64",
    runner: "blacksmith-12vcpu-macos-latest",
    windows: false,
    describes: "macOS on Apple silicon",
  },
  {
    npm: "@slidxjs/cli-darwin-x64",
    target: "x86_64-apple-darwin",
    os: "darwin",
    cpu: "x64",
    runner: "blacksmith-12vcpu-macos-latest",
    windows: false,
    describes: "macOS on Intel",
  },
  {
    npm: "@slidxjs/cli-linux-x64",
    target: "x86_64-unknown-linux-musl",
    os: "linux",
    cpu: "x64",
    runner: "blacksmith-32vcpu-ubuntu-2404",
    windows: false,
    describes: "Linux on x86-64",
  },
  {
    npm: "@slidxjs/cli-linux-arm64",
    target: "aarch64-unknown-linux-musl",
    os: "linux",
    cpu: "arm64",
    runner: "blacksmith-32vcpu-ubuntu-2404-arm",
    windows: false,
    describes: "Linux on ARM64",
  },
  {
    npm: "@slidxjs/cli-win32-x64",
    target: "x86_64-pc-windows-msvc",
    os: "win32",
    cpu: "x64",
    runner: "blacksmith-32vcpu-windows-2025",
    windows: true,
    describes: "Windows on x86-64",
  },
];

/** The name of the file inside the archive, and inside the npm package. */
export function binaryName(platform) {
  return platform.windows ? "slidx.exe" : "slidx";
}

/** The release asset for one platform. Named by target triple, so a version
 * manager can construct it without a lookup table of its own. */
export function assetName(platform) {
  return `slidx-${platform.target}.${platform.windows ? "zip" : "tar.gz"}`;
}

/**
 * The one file that says what every asset should hash to.
 *
 * Named for the format rather than the release, so a downloader can build the
 * URL from a version alone. The contents are `sha256sum -c` format, which is
 * what makes verification a one-liner in the installer rather than a parser.
 */
export const CHECKSUM_FILE = "SHA256SUMS";

/** Platforms an installer running on a POSIX shell can reach. */
export function posixPlatforms() {
  return PLATFORMS.filter((platform) => !platform.windows);
}
