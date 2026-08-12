#!/usr/bin/env bash
set -euo pipefail

if command -v apt-get >/dev/null 2>&1; then
    sudo apt-get update && sudo apt-get install -y libdbus-1-dev libwayland-dev libxkbcommon-dev libssl-dev pkg-config libudev-dev
fi

if [ ! -d "../idle" ] && [ "$(basename "$PWD")" != "idle" ]; then git clone https://github.com/idlescreen/idle ../idle; fi

if ! command -v rustup >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    if [ -f "$HOME/.cargo/env" ]; then
        # shellcheck source=/dev/null
        source "$HOME/.cargo/env"
    fi
fi

if [ -f "rust-toolchain.toml" ] || [ -f "rust-toolchain" ]; then
    rustup show
else
    rustup default 1.96.0
fi

cargo test
