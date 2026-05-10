#!/usr/bin/env bash
# Copy + rename Tauri bundle outputs into a staging dir using amd64/arm64 suffix.
#
# Usage: stage-bundles.sh <rust-target-triple> <matrix-label> <staging-dir>
#
# Mapping (only the formats we want to ship):
#   macos-arm64   -> dmg + app.tar.gz (+ .sig), aarch64 → arm64
#   macos-x64     -> dmg + app.tar.gz (+ .sig), x64/x86_64 → amd64
#   linux-x64     -> .deb (+ .sig), already amd64
#   linux-arm64   -> .deb (+ .sig), already arm64
#   windows-x64   -> NSIS exe (+ .sig), x64-setup.exe → amd64.exe

set -euo pipefail
shopt -s nullglob

TARGET="$1"
LABEL="$2"
STAGE="$3"

mkdir -p "$STAGE"
BUNDLE="src-tauri/target/$TARGET/release/bundle"

stage() {
  local src="$1"
  local newname="$2"
  echo "stage: $(basename "$src") -> $newname"
  cp "$src" "$STAGE/$newname"
}

case "$LABEL" in
  macos-arm64)
    for src in "$BUNDLE"/dmg/*.dmg; do
      name=$(basename "$src")
      stage "$src" "${name//aarch64/arm64}"
    done
    for src in "$BUNDLE"/macos/*.app.tar.gz "$BUNDLE"/macos/*.app.tar.gz.sig; do
      name=$(basename "$src")
      stage "$src" "${name//aarch64/arm64}"
    done
    ;;
  macos-x64)
    for src in "$BUNDLE"/dmg/*.dmg; do
      name=$(basename "$src")
      newname="${name//x86_64/amd64}"
      newname="${newname//x64/amd64}"
      stage "$src" "$newname"
    done
    for src in "$BUNDLE"/macos/*.app.tar.gz "$BUNDLE"/macos/*.app.tar.gz.sig; do
      name=$(basename "$src")
      newname="${name//x86_64/amd64}"
      newname="${newname//x64/amd64}"
      stage "$src" "$newname"
    done
    ;;
  linux-x64|linux-arm64)
    for src in "$BUNDLE"/deb/*.deb "$BUNDLE"/deb/*.deb.sig; do
      stage "$src" "$(basename "$src")"
    done
    ;;
  windows-x64)
    for src in "$BUNDLE"/nsis/*-setup.exe "$BUNDLE"/nsis/*-setup.exe.sig; do
      name=$(basename "$src")
      newname="${name//x64-setup.exe/amd64.exe}"
      stage "$src" "$newname"
    done
    ;;
  *)
    echo "unknown matrix label: $LABEL" >&2
    exit 1
    ;;
esac

echo "--- staged in $STAGE ---"
ls -la "$STAGE"
