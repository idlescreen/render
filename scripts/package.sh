#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Build signed-ready RPM/DEB for render into packages pool if present.
# (studio moved to the standalone idlescreen/idle-studio repo — package it there.)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

./scripts/qa_package_gate.sh

echo ">>> release build"
cargo build --release -p render

echo ">>> generate RPM"
(cd engine && cargo generate-rpm)

echo ">>> generate DEB"
# cargo-deb resolves assets relative to package; workspace bins live in ../target
(cd engine && cargo deb --no-build) || cargo deb -p render --no-build

POOL_RPM="${PACKAGES_POOL_RPM:-$ROOT/../packages/rpm/pool}"
POOL_DEB="${PACKAGES_POOL_DEB:-$ROOT/../packages/apt/pool/main}"
if [[ -d "$POOL_RPM" ]]; then
  cp -f target/generate-rpm/render-*.rpm engine/target/generate-rpm/render-*.rpm "$POOL_RPM/" 2>/dev/null || true
  # Prefer newest matches only
  find target engine/target -name 'render-*.rpm' 2>/dev/null \
    | while read -r f; do cp -f "$f" "$POOL_RPM/"; done
  echo "    RPM → $POOL_RPM"
  ls -la "$POOL_RPM"/render-*.rpm 2>/dev/null | tail -10
fi
if [[ -d "$POOL_DEB" ]]; then
  find target engine/target debian -name 'render_*.deb' 2>/dev/null \
    | while read -r f; do cp -f "$f" "$POOL_DEB/"; done || true
  echo "    DEB → $POOL_DEB"
fi
echo "PACKAGE_PASS"
