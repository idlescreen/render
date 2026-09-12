#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Headless package gate for render + idle-studio (no Wayland session required).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ "${SKIP_TESTS:-}" == "1" || "${SKIP_TESTS:-}" == "true" ]]; then
  echo "SKIP_TESTS set — render package gate skipped."
  exit 0
fi

if [[ ! -e idle ]]; then
  if [[ -d ../idle ]]; then
    ln -sfn ../idle idle
  else
    echo "FAIL: need sibling ../idle or ./idle symlink" >&2
    exit 1
  fi
fi

echo "=========================================="
echo "Render package gate"
echo "=========================================="
echo ">>> cargo test --workspace"
cargo test --workspace --quiet
echo ">>> dry-run render (plan only)"
cargo build --release -p render -q
./target/release/render -e beams --duration 1s --dry-run
if command -v ffmpeg >/dev/null 2>&1 && [[ -f ../idle-savers/beams/target/release/libscreensaver_beams.so ]]; then
  echo ">>> snapshot compare (PNG last frame vs baseline)"
  IDLE_ALLOW_UNSIGNED_PLUGINS=1 \
    ./target/release/render \
      --plugin-path "$(cd .. && pwd)/idle-savers/beams/target/release/libscreensaver_beams.so" \
      -e beams --seed 3735928559 --duration 2s \
      --format png --cpu-raster \
      --baseline-dir engine/tests/snapshots/baselines \
      --snapshot-last-only \
      -o /tmp/qa-snap.png
fi
echo "RENDER_PACKAGE_GATE_PASS"
