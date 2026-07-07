#!/usr/bin/env bash
# Regenerate docs/provenant-demo.gif — the Provenant-vs-ScanCode race in the README.
#
# Usage (from anywhere in the repo):
#   docs/demo/record.sh
#
# One-time host setup:  brew install vhs tmux   (plus a running Docker daemon)
#
# This script is idempotent: it clones astral-sh/uv and builds the ScanCode Docker image
# only if they are missing. Everything transient lives under .provenant/ (gitignored), so
# regenerating never touches tracked files. See docs/demo.tape for the recording script.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

for tool in vhs tmux docker; do
  command -v "$tool" >/dev/null || { echo "error: '$tool' not found (brew install vhs tmux; start Docker)"; exit 1; }
done

echo "==> building release provenant"
cargo build --release --bin provenant

demo=.provenant/demo
mkdir -p "$demo"
if [ ! -d "$demo/uv" ]; then
  echo "==> cloning astral-sh/uv into $demo/uv"
  git clone --depth 1 https://github.com/astral-sh/uv "$demo/uv"
  rm -rf "$demo/uv/.git"
fi

# The demo must reflect the committed ScanCode version. A dirty submodule would make
# compare-outputs build a differently-tagged (`-dirty-<hash>`) image that the clean-tag
# reuse check below can't match, so require a clean submodule for a reproducible demo.
if [ -n "$(git -C reference/scancode-toolkit status --porcelain)" ]; then
  echo "error: reference/scancode-toolkit has uncommitted changes; commit or stash them first"; exit 1
fi

# Match the image built for the *current* ScanCode submodule commit (mirrors the tag
# xtask compare-outputs derives) so we never silently re-record against a stale image.
short_commit="$(git -C reference/scancode-toolkit rev-parse HEAD | cut -c1-10)"
case "$(docker info --format '{{.Architecture}}' 2>/dev/null || uname -m)" in
  arm64 | aarch64) platform_label="linux-arm64-v8" ;;
  *) platform_label="linux-amd64" ;;
esac
image="provenant-scancode-local:${short_commit}-${platform_label}"
if ! docker image inspect "$image" >/dev/null 2>&1; then
  echo "==> building the ScanCode Docker image $image (one-time, via xtask compare-outputs)"
  cargo run --manifest-path xtask/Cargo.toml --bin compare-outputs -- \
    --target-path "$demo/uv" --profile common || true
  docker image inspect "$image" >/dev/null 2>&1 || { echo "error: ScanCode image $image was not built"; exit 1; }
fi
echo "==> using ScanCode image: $image"

# Put the scancode wrapper on PATH and give it the image + a writable output dir. The
# unique run label lets cleanup target only the container this recording started, never a
# concurrent compare run or manual scan sharing the same image.
bin="$(mktemp -d)"
ln -sf "$PWD/docs/demo/scancode" "$bin/scancode"
export PATH="$bin:$PATH"
export SCANCODE_IMAGE="$image"
export SCANCODE_OUT="$(mktemp -d)"
export SCANCODE_RUN_LABEL="provenant-demo-record=$$"

echo "==> recording docs/provenant-demo.gif"
vhs docs/demo.tape

# Stop only the ScanCode container this recording started (matched by its unique label),
# in case it is still scanning when the recording ends.
leftover="$(docker ps --filter "label=$SCANCODE_RUN_LABEL" --quiet)"
[ -n "$leftover" ] && docker kill $leftover >/dev/null 2>&1 || true

echo "==> done: docs/provenant-demo.gif"
