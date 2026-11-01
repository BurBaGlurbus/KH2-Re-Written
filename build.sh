#!/bin/sh
# Builds the launcher on Linux, the same one build.bat builds on Windows: with
# the skate3 feature, optimized, named "KH2RW Launcher" (Cargo cannot name a
# binary with a space). The game and the model/image viewers run inside it.
#   ./build.sh            optimized, with Skate 3
#   ./build.sh --debug    the faster development profile
#   ./build.sh --no-skate3
# System packages and the rest of the setup: docs/LINUX.md.
set -eu

cd "$(dirname "$0")"
PATH="$HOME/.cargo/bin:$PATH"

profile=release
folder=release
features="--features skate3"
for arg in "$@"; do
    case "$arg" in
        --debug)
            profile=dev
            folder=debug
            ;;
        --no-skate3) features="" ;;
        *)
            echo "unknown option: $arg (use --debug or --no-skate3)" >&2
            exit 2
            ;;
    esac
done

if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo is not installed; get it from https://rustup.rs" >&2
    exit 1
fi
if command -v pkg-config >/dev/null 2>&1; then
    for library in alsa libudev wayland-client xkbcommon; do
        if ! pkg-config --exists "$library"; then
            echo "missing the $library development package; see docs/LINUX.md" >&2
            exit 1
        fi
    done
else
    echo "pkg-config is not installed; see docs/LINUX.md" >&2
    exit 1
fi

# shellcheck disable=SC2086 # $features is one option and its value, or nothing.
cargo build -p main --bin KH2RW $features --profile "$profile" --locked

output="${CARGO_TARGET_DIR:-target}/$folder"
mv -f "$output/KH2RW" "$output/KH2RW Launcher"
echo "Built $output/KH2RW Launcher"
