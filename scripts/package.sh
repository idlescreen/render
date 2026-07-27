#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Build signed-ready RPM/DEB for render + idle-studio into packages pool if present.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

./scripts/qa_package_gate.sh

echo ">>> release build"
cargo build --release -p render -p idle-studio

echo ">>> generate RPM"
(cd engine && cargo generate-rpm)
(cd studio && cargo generate-rpm)

echo ">>> generate DEB"
# cargo-deb resolves assets relative to package; workspace bins live in ../target
(cd engine && cargo deb --no-build) || cargo deb -p render --no-build
(cd studio && cargo deb --no-build) || cargo deb -p idle-studio --no-build

POOL_RPM="${PACKAGES_POOL_RPM:-$ROOT/../packages/rpm/pool}"
POOL_DEB="${PACKAGES_POOL_DEB:-$ROOT/../packages/apt/pool/main}"
if [[ -d "$POOL_RPM" ]]; then
  cp -f target/generate-rpm/render-*.rpm engine/target/generate-rpm/render-*.rpm "$POOL_RPM/" 2>/dev/null || true
  cp -f target/generate-rpm/idle-studio-*.rpm studio/target/generate-rpm/idle-studio-*.rpm "$POOL_RPM/" 2>/dev/null || true
  # Prefer newest matches only
  find target engine/target studio/target -name 'render-*.rpm' -o -name 'idle-studio-*.rpm' 2>/dev/null \
    | while read -r f; do cp -f "$f" "$POOL_RPM/"; done
  echo "    RPM → $POOL_RPM"
  ls -la "$POOL_RPM"/render-*.rpm "$POOL_RPM"/idle-studio-0.3*.rpm 2>/dev/null || ls -la "$POOL_RPM"/*studio* "$POOL_RPM"/render* 2>/dev/null | tail -10
fi
if [[ -d "$POOL_DEB" ]]; then
  find target engine/target studio/target debian -name 'render_*.deb' -o -name 'idle-studio_*.deb' 2>/dev/null \
    | while read -r f; do cp -f "$f" "$POOL_DEB/"; done || true
  echo "    DEB → $POOL_DEB"
fi
echo "PACKAGE_PASS"
