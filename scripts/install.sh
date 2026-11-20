#!/bin/sh
# Installs the `vnoc` command-line program and the Python driver beside it, from a V-NOC release (docs/21-agent-cli/07-install.md §6):
#
#   curl -fsSL https://github.com/yaredtsy/vnoc/releases/download/v0.3.0/install.sh | sh
#
# It picks the build for this system, downloads the release's archive and SHA256SUMS, checks the sum, and only then writes anything into the install
# folder. POSIX sh; no bash, no sudo; it writes nowhere but the install folder (and a temporary folder it removes). No Windows script: the release's
# .zip and the VS Code command *Install `vnoc` Command in PATH* cover Windows.
#
# What it reads from the environment:
#   VNOC_VERSION      the release to install: 0.3.0 or v0.3.0. Default: the version stamped into this script when the release was made; run from a
#                     checkout (not stamped), the newest release, pre-releases included
#   VNOC_INSTALL_DIR  where the two programs go. Default: $HOME/.local/bin (made if it is not there)
#   VNOC_BASE_URL     where the releases are, instead of https://github.com/yaredtsy/vnoc/releases/download: a mirror, or an offline machine's folder
#                     (`file:///srv/vnoc` needs curl; the folder holds v<version>/ with the archive and SHA256SUMS in it)
set -eu

# The release's `publish` job replaces the next line with the release's version; never edit it by hand. It stays a top-level line: the stamp looks for `^VERSION=`.
VERSION="@VERSION@"

# Everything else is one function, called on the last line: a download that is cut off (`curl … | sh` stopping halfway) ends inside the function, which
# the shell cannot parse, so it runs nothing at all.
main() {
  REPO="yaredtsy/vnoc"
  BASE="${VNOC_BASE_URL:-https://github.com/$REPO/releases/download}"
  BASE="${BASE%/}"
  TARGETS="darwin-arm64, darwin-x64, linux-x64, win32-x64"

  say() { printf '%s\n' "$*"; }
  fail() {
    printf 'error: %s\n' "$*" >&2
    exit 1
  }

  work=""
  dir=""
  cleanup() {
    # the temporary folder, and any half-made file beside the programs (a program is moved into place only when it is whole)
    [ -z "$work" ] || rm -rf "$work"
    [ -z "$dir" ] || rm -f "$dir/.vnoc.new.$$" "$dir/.vnoc-driver-python.new.$$"
  }
  trap cleanup EXIT
  trap 'exit 1' HUP INT TERM

  have() { command -v "$1" >/dev/null 2>&1; }

  # ── the target ───────────────────────────────────────────────────────────────────────────────────────────────────────────

  os=$(uname -s)
  arch=$(uname -m)
  case "$os $arch" in
    "Darwin arm64") target=darwin-arm64 ;;
    "Darwin x86_64")
      # a shell running under Rosetta reports x86_64 on an Apple silicon Mac: the native build is the one to install
      if [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = "1" ]; then target=darwin-arm64; else target=darwin-x64; fi
      ;;
    "Linux x86_64") target=linux-x64 ;;
    *) fail "no vnoc build for $os $arch; the releases have $TARGETS" ;;
  esac

  # ── the tools ────────────────────────────────────────────────────────────────────────────────────────────────────────────

  if have curl; then
    # fetch URL: the body on standard output
    fetch() { curl -fsSL "$1"; }
  elif have wget; then
    fetch() { wget -qO- "$1"; }
  else
    fail "neither curl nor wget is installed: install one of them, then run this again"
  fi
  have tar || fail "tar is not installed: install it, then run this again"
  if have sha256sum; then
    sum_of() { sha256sum "$1" | cut -d ' ' -f 1; }
  elif have shasum; then
    sum_of() { shasum -a 256 "$1" | cut -d ' ' -f 1; }
  else
    fail "neither sha256sum nor shasum is installed, so the download cannot be checked: install one of them, then run this again"
  fi

  # ── the version ──────────────────────────────────────────────────────────────────────────────────────────────────────────

  version="${VNOC_VERSION:-}"
  if [ -z "$version" ] && [ "$VERSION" != "@VERSION@" ]; then
    version="$VERSION"
  fi
  if [ -z "$version" ]; then
    # run from a checkout: the newest release. Every V-NOC release so far is a pre-release, which `releases/latest` skips, so ask for the list
    api="https://api.github.com/repos/$REPO/releases?per_page=1"
    listing=$(fetch "$api") || fail "could not ask $api for the newest release: check the network, or name one: VNOC_VERSION=0.3.0 sh install.sh"
    version=$(printf '%s\n' "$listing" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
    [ -n "$version" ] || fail "$api names no release: name one: VNOC_VERSION=0.3.0 sh install.sh"
  fi
  version="${version#v}"

  archive="vnoc-cli-$target.tar.gz"
  release="$BASE/v$version"

  # ── the download, and the check, before anything is written to the install folder ──────────────────────────────────────────

  work=$(mktemp -d "${TMPDIR:-/tmp}/vnoc-install.XXXXXX") || fail "could not make a temporary folder in ${TMPDIR:-/tmp}"
  say "vnoc: installing $version for $target"

  fetch "$release/SHA256SUMS" >"$work/SHA256SUMS" ||
    fail "could not download $release/SHA256SUMS: is v$version a release? See https://github.com/$REPO/releases"
  want=$(awk -v n="$archive" '{ f = $2; sub(/^\*/, "", f); if (f == n) { print $1; exit } }' "$work/SHA256SUMS")
  [ -n "$want" ] || fail "no vnoc-cli archive in release v$version: its SHA256SUMS has no $archive. Releases before 0.3.0 have none; try a newer one: VNOC_VERSION=0.3.0 sh install.sh"

  fetch "$release/$archive" >"$work/$archive" || fail "could not download $release/$archive"
  got=$(sum_of "$work/$archive")
  [ -n "$got" ] || fail "could not work out the sum of $archive: nothing was installed"
  [ "$got" = "$want" ] || fail "$archive does not match its line of SHA256SUMS (wanted $want, got $got): nothing was installed. Try again; if it keeps failing, the download is damaged or not the release's"

  mkdir "$work/unpack"
  tar -xzf "$work/$archive" -C "$work/unpack" vnoc vnoc-driver-python || fail "could not unpack $archive: it does not hold vnoc and vnoc-driver-python"

  # ── the install: each program under a temporary name beside its place, then moved there ───────────────────────────────────

  dir="${VNOC_INSTALL_DIR:-$HOME/.local/bin}"
  mkdir -p "$dir" || fail "could not make $dir: pick another folder with VNOC_INSTALL_DIR=<folder>"
  # VS Code's *Install `vnoc` Command in PATH* makes `vnoc` a link into the extension: moving a program over a link replaces the link, not what it points to
  replaced_link=""
  if [ -L "$dir/vnoc" ]; then replaced_link=$(readlink "$dir/vnoc" 2>/dev/null || true); fi
  for name in vnoc-driver-python vnoc; do
    # the driver first: a vnoc that is moved in finds its driver beside it
    case "$name" in vnoc) new="$dir/.vnoc.new.$$" ;; *) new="$dir/.vnoc-driver-python.new.$$" ;; esac
    cp "$work/unpack/$name" "$new" || fail "could not write into $dir: pick another folder with VNOC_INSTALL_DIR=<folder>"
    chmod 755 "$new"
    mv -f "$new" "$dir/$name"
  done

  installed=$("$dir/vnoc" --version) || fail "$dir/vnoc was installed but does not run on this system"
  say "$installed"
  say "installed vnoc and vnoc-driver-python in $dir"
  if [ -n "$replaced_link" ]; then
    say "replaced the link to $replaced_link (VS Code's *Install vnoc Command in PATH*): this copy is the one on the PATH now"
  fi

  # ── after ────────────────────────────────────────────────────────────────────────────────────────────────────────────────

  # the lines for the user to type: `$HOME` and `~` are meant literally in them
  # shellcheck disable=SC2016,SC2088
  case ":$PATH:" in
    *":$dir:"*) ;;
    *)
      shown="$dir"
      if [ "$dir" = "$HOME/.local/bin" ]; then shown='$HOME/.local/bin'; fi
      case "$(basename "${SHELL:-sh}")" in
        zsh) rc="~/.zshrc" ;;
        bash) rc="~/.bashrc (~/.bash_profile on macOS)" ;;
        *) rc="your shell's startup file" ;;
      esac
      say ""
      say "$dir is not on your PATH. Add this line to $rc, then open a new terminal:"
      say "  export PATH=\"$shown:\$PATH\""
      ;;
  esac
}

main "$@"
