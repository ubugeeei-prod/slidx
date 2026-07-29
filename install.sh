#!/bin/sh
#
# slidx installer.
#
#   curl -fsSL https://raw.githubusercontent.com/ubugeeei-prod/slidx/main/install.sh | sh
#
# Downloads the prebuilt binary for this machine, checks it against the
# published SHA-256, and puts it under $HOME. No sudo, no compiler, no Node.
#
# ------------------------------------------------------------------------------
# Four things this script does on purpose
# ------------------------------------------------------------------------------
#
# 1. EVERYTHING IS A FUNCTION, AND main IS CALLED ON THE LAST LINE.
#
#    `curl | sh` feeds the script to the shell as it arrives. If the connection
#    drops halfway, a script written as a list of statements has already run
#    half of them — which for an installer means a half-extracted binary on the
#    PATH. Defining functions and calling main at the very end means a truncated
#    download does nothing at all.
#
# 2. THE CHECKSUM IS NOT OPTIONAL.
#
#    A pipe-to-shell installer that does not verify what it downloaded is the
#    exact thing people are right to be suspicious of. If the checksum file is
#    missing, if this platform is not in it, or if the hashes differ, the script
#    stops and deletes what it fetched.
#
#    Be clear about what that does and does not buy: it proves the file arrived
#    intact and is the file the release published. Both come from the same
#    server, so it is not a defence against a compromised GitHub account — for
#    that, verify the build attestation, which the release also publishes:
#
#        gh attestation verify slidx-<target>.tar.gz --repo ubugeeei-prod/slidx
#
# 3. AN UNKNOWN PLATFORM IS AN ERROR, NOT A GUESS.
#
#    Installing a binary that will not run, and finding out at a lectern, is
#    worse than not installing one. The failure names every platform that is
#    published and how to build from source.
#
# 4. IT SAYS WHERE IT PUT THINGS.
#
#    Including whether that directory is on your PATH, and — the one that costs
#    people an hour — whether a *different* slidx is going to win.
#
# ------------------------------------------------------------------------------
# Knobs
# ------------------------------------------------------------------------------
#
#   SLIDX_VERSION   tag to install, e.g. v0.1.0. Default: the latest release.
#   SLIDX_HOME      install root. Default: $XDG_DATA_HOME/slidx, else ~/.slidx.
#   SLIDX_OS        override the detected operating system (uname -s).
#   SLIDX_ARCH      override the detected architecture (uname -m).
#   SLIDX_BASE_URL  where to fetch the archive and SHA256SUMS from, for an
#                   internal mirror or an air-gapped copy. A file:// URL works.
#                   The checksum is still checked — a mirror is exactly the
#                   place a file is most likely to be stale or wrong.
#
#   --dry-run       say what would happen and download nothing.
#   --version <v>   same as SLIDX_VERSION.
#   --help
#
set -eu

REPO="ubugeeei-prod/slidx"
CHECKSUM_FILE="SHA256SUMS"

# uname -s | uname -m | rust target | what to call it.
#
# Pipe-delimited rather than aligned on spaces, because the last column is
# prose: splitting on whitespace would turn "macOS on Intel" into three fields
# and print the platform list as gibberish at the exact moment somebody needs
# to read it.
#
# Kept in step with scripts/platforms.mjs by a test — a platform this script
# cannot name is a platform nobody can install with it.
PLATFORMS="\
Darwin|arm64|aarch64-apple-darwin|macOS on Apple silicon
Darwin|x86_64|x86_64-apple-darwin|macOS on Intel
Linux|x86_64|x86_64-unknown-linux-musl|Linux on x86-64
Linux|aarch64|aarch64-unknown-linux-musl|Linux on ARM64"

DRY_RUN=0

main() {
    parse_arguments "$@"

    target=$(detect_target)
    version=${SLIDX_VERSION:-}
    home=$(install_home)
    bin_dir="$home/bin"
    asset="slidx-$target.tar.gz"

    if [ "$DRY_RUN" = "1" ]; then
        report_plan "$target" "$asset" "${version:-latest}" "$bin_dir"
        return 0
    fi

    need curl_or_wget
    need sha256

    tmp=$(mktemp -d 2>/dev/null || mktemp -d -t slidx)
    # Runs on the way out however this ends, so a failed verification never
    # leaves a downloaded binary lying in /tmp for somebody to run by hand.
    trap 'rm -rf "$tmp"' EXIT INT TERM

    base=$(release_url "$version")

    say "downloading $asset"
    fetch "$base/$asset" "$tmp/$asset"
    fetch "$base/$CHECKSUM_FILE" "$tmp/$CHECKSUM_FILE"

    verify "$tmp" "$asset"
    say "checksum ok"

    tar -xzf "$tmp/$asset" -C "$tmp"
    [ -f "$tmp/slidx" ] || die "the archive did not contain a slidx binary"

    mkdir -p "$bin_dir"
    # Moved into place rather than copied over: replacing a running binary in
    # place is what makes "text file busy" happen halfway through an upgrade.
    mv -f "$tmp/slidx" "$bin_dir/slidx"
    chmod +x "$bin_dir/slidx"

    report_installed "$bin_dir"
}

parse_arguments() {
    while [ $# -gt 0 ]; do
        case "$1" in
        --dry-run) DRY_RUN=1 ;;
        --version)
            [ $# -ge 2 ] || die "--version needs a tag, for example --version v0.1.0"
            SLIDX_VERSION="$2"
            shift
            ;;
        --version=*) SLIDX_VERSION="${1#--version=}" ;;
        --help | -h)
            usage
            exit 0
            ;;
        *) die "$1 is not an option of this installer. Try --help." ;;
        esac
        shift
    done
}

# Where the binary goes.
#
# Under $HOME, always, so this never needs sudo — a script people pipe into a
# shell must not also ask for root. The path is the one `slidx version` will
# manage later, so the installer and the version manager agree from the start
# rather than fighting over two directories.
install_home() {
    if [ -n "${SLIDX_HOME:-}" ]; then
        printf '%s' "$SLIDX_HOME"
    elif [ -n "${XDG_DATA_HOME:-}" ]; then
        printf '%s/slidx' "$XDG_DATA_HOME"
    else
        printf '%s/.slidx' "$HOME"
    fi
}

detect_target() {
    os=${SLIDX_OS:-$(uname -s)}
    arch=${SLIDX_ARCH:-$(uname -m)}

    # uname reports the architecture of the *process*, and a shell started under
    # Rosetta says x86_64 on a machine that is not. Installing the Intel build
    # there works and is slower for no reason, so ask the kernel instead.
    if [ "$os" = "Darwin" ] && [ "$arch" = "x86_64" ] &&
        [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || echo 0)" = "1" ]; then
        arch=arm64
    fi

    # The spellings other tools use for the same machine.
    case "$arch" in
    amd64 | x64) arch=x86_64 ;;
    arm64 | armv8* | aarch64) [ "$os" = "Darwin" ] && arch=arm64 || arch=aarch64 ;;
    esac

    printf '%s\n' "$PLATFORMS" | while IFS= read -r row; do
        row_os=$(field "$row" 1)
        row_arch=$(field "$row" 2)
        if [ "$row_os" = "$os" ] && [ "$row_arch" = "$arch" ]; then
            field "$row" 3
            return 0
        fi
    done | grep . || unsupported "$os" "$arch"
}

field() {
    printf '%s\n' "$1" | cut -d'|' -f"$2"
}

unsupported() {
    printf 'slidx: no prebuilt binary for %s %s.\n\n' "$1" "$2" >&2
    printf 'Prebuilt binaries exist for:\n' >&2
    printf '%s\n' "$PLATFORMS" | while IFS= read -r row; do
        printf '  %-28s %s\n' "$(field "$row" 3)" "$(field "$row" 4)" >&2
    done
    printf '\nWindows: install with `npm i -g slidx` instead.\n' >&2
    printf 'Anything else: `cargo install slidx_cli` builds it from source.\n' >&2
    exit 1
}

# The directory a release's assets live under.
release_url() {
    if [ -n "${SLIDX_BASE_URL:-}" ]; then
        printf '%s' "${SLIDX_BASE_URL%/}"
    elif [ -n "$1" ]; then
        printf 'https://github.com/%s/releases/download/%s' "$REPO" "$1"
    else
        # Redirects to the newest tag, so no API call and no rate limit.
        printf 'https://github.com/%s/releases/latest/download' "$REPO"
    fi
}

# Compares the downloaded file against the published hash.
#
# An asset the checksum file does not mention is a failure, not a pass: that is
# what an installer pointed at a release built before this platform existed
# would otherwise do, silently.
verify() {
    expected=$(awk -v want="$2" '$2 == want || $2 == "*" want {print $1}' "$1/$CHECKSUM_FILE" | head -n 1)
    [ -n "$expected" ] || die "$CHECKSUM_FILE does not list $2, so it cannot be verified"

    actual=$(sha256 "$1/$2")
    [ "$expected" = "$actual" ] || die "checksum mismatch for $2
  published $expected
  got       $actual
Nothing was installed. Fetch it again, and if it happens twice, open an issue."
}

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    else
        return 1
    fi
}

fetch() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$1" -o "$2" || die "could not download $1"
    else
        wget -qO "$2" "$1" || die "could not download $1"
    fi
}

curl_or_wget() {
    command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1
}

need() {
    case "$1" in
    curl_or_wget) curl_or_wget || die "this needs curl or wget on PATH" ;;
    sha256) sha256 /dev/null >/dev/null 2>&1 || die "this needs sha256sum or shasum on PATH, because the download is verified and that is not optional" ;;
    esac
}

report_plan() {
    printf 'slidx would install:\n\n'
    printf '  platform  %s\n' "$1"
    printf '  asset     %s\n' "$2"
    printf '  version   %s\n' "$3"
    printf '  from      %s\n' "$(release_url "${SLIDX_VERSION:-}")"
    printf '  into      %s/slidx\n' "$4"
    printf '  verified  against %s from the same release\n' "$CHECKSUM_FILE"
    printf '\nNothing was downloaded. Run without --dry-run to install.\n'
}

# What happened, where it is, and what will actually run when you type `slidx`.
report_installed() {
    installed=$("$1/slidx" --version 2>/dev/null || echo slidx)

    printf '\n%s\n' "$installed"
    printf 'installed to %s/slidx\n\n' "$1"

    case ":$PATH:" in
    *":$1:"*)
        found=$(command -v slidx 2>/dev/null || true)
        if [ -n "$found" ] && [ "$found" != "$1/slidx" ]; then
            # The hour-long confusion this line exists to prevent: `npm i -g
            # slidx` earlier on PATH, and every fix applied to the wrong binary.
            printf 'Careful: `slidx` still resolves to %s, which comes earlier on\n' "$found"
            printf 'your PATH. Remove that one, or put %s ahead of it.\n' "$1"
        else
            printf 'Run `slidx doctor` before your next talk.\n'
        fi
        ;;
    *)
        printf '%s is not on your PATH. Add it:\n\n' "$1"
        printf '  export PATH="%s:$PATH"\n\n' "$1"
        printf 'Put that line in your shell profile to keep it.\n'
        ;;
    esac
}

# Spelled out rather than read back out of this file.
#
# `curl | sh` leaves $0 as `sh` and the script itself on standard input, so
# anything that reads its own source prints the wrong thing or nothing at all —
# and that is the one way most people will ever run this.
usage() {
    cat <<'USAGE'
slidx installer

  curl -fsSL https://raw.githubusercontent.com/ubugeeei-prod/slidx/main/install.sh | sh

Downloads the prebuilt slidx for this machine, verifies it against the SHA-256
published with the release, and installs it under your home directory. No sudo,
no compiler, no Node.

Options:
  --dry-run        say what would happen and download nothing
  --version <tag>  install a specific release, e.g. v0.1.0
  -h, --help       print this

Environment:
  SLIDX_VERSION    same as --version
  SLIDX_HOME       install root. Default: $XDG_DATA_HOME/slidx, else ~/.slidx
  SLIDX_OS         override the detected operating system (uname -s)
  SLIDX_ARCH       override the detected architecture (uname -m)
  SLIDX_BASE_URL   fetch from a mirror or a local file:// copy. Still verified

Windows is published on npm rather than here: npm i -g slidx
Anywhere with no prebuilt binary: cargo install slidx_cli
USAGE
}

say() {
    printf 'slidx: %s\n' "$1" >&2
}

die() {
    printf 'slidx: %s\n' "$1" >&2
    exit 1
}

main "$@"
