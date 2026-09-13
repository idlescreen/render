#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Build signed-ready RPM/DEB for render + idle-studio into packages pool if present.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

./scripts/qa_package_gate.sh

echo ">>> release build"
cargo build --release -p render -p idle-studio

for member in . engine; do
  echo ">>> generate RPM ($member)"
  (cd "$member" && cargo generate-rpm)
  echo ">>> generate DEB ($member)"
  # cargo-deb resolves assets relative to package; workspace bins live in ../target
  (cd "$member" && cargo deb --no-build)
done

POOL_RPM="${PACKAGES_POOL_RPM:-$ROOT/../packages/rpm/pool}"
POOL_DEB="${PACKAGES_POOL_DEB:-$ROOT/../packages/apt/pool/main}"
if [[ -d "$POOL_RPM" ]]; then
  find target engine/target studio/target -name '*.rpm' 2>/dev/null \
    | while read -r f; do cp -f "$f" "$POOL_RPM/"; done
  echo "    RPM → $POOL_RPM"
  ls -la "$POOL_RPM"/*.rpm 2>/dev/null | tail -10
fi
if [[ -d "$POOL_DEB" ]]; then
  find target engine/target studio/target -name '*.deb' 2>/dev/null \
    | while read -r f; do cp -f "$f" "$POOL_DEB/"; done || true
  echo "    DEB → $POOL_DEB"
fi
echo "PACKAGE_PASS"
