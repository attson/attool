#!/usr/bin/env bash
# Copy + rename Tauri bundle outputs into a staging dir using amd64/arm64 suffix.
#
# Usage: stage-bundles.sh <rust-target-triple> <matrix-label> <staging-dir>
#
# Mapping (only the formats we want to ship):
#   macos-arm64   -> dmg + app.tar.gz (+ .sig), aarch64 → arm64
#   macos-x64     -> dmg + app.tar.gz (+ .sig), x64/x86_64 → amd64
#   linux-x64     -> .deb (no updater sig — Tauri does not sign deb)
#   linux-arm64   -> .deb
#   windows-x64   -> NSIS exe (user download) + .nsis.zip (+ .sig) (updater)
#                    *-setup.exe → _amd64.exe ; *-setup.nsis.zip → _amd64.nsis.zip

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
      # Tauri may emit "AT Tool.app.tar.gz" (no arch suffix) or "AT Tool_aarch64.app.tar.gz".
      # Normalize to *_arm64.app.tar.gz for consistency.
      newname="${name//aarch64/arm64}"
      if [[ "$newname" != *_arm64* ]]; then
        # No arch suffix in original — inject _arm64 before .app.tar.gz
        newname="${newname%.app.tar.gz}_arm64.app.tar.gz${name##*.app.tar.gz}"
      fi
      stage "$src" "$newname"
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
      if [[ "$newname" != *_amd64* ]]; then
        newname="${newname%.app.tar.gz}_amd64.app.tar.gz${name##*.app.tar.gz}"
      fi
      stage "$src" "$newname"
    done
    ;;
  linux-x64|linux-arm64)
    for src in "$BUNDLE"/deb/*.deb "$BUNDLE"/deb/*.deb.sig; do
      stage "$src" "$(basename "$src")"
    done
    ;;
  windows-x64)
    # User-facing installer
    for src in "$BUNDLE"/nsis/*-setup.exe; do
      name=$(basename "$src")
      stage "$src" "${name//x64-setup.exe/amd64.exe}"
    done
    # Updater artifact (signed). Tauri 2 with createUpdaterArtifacts emits
    # *-setup.nsis.zip (+ .sig).
    for src in "$BUNDLE"/nsis/*-setup.nsis.zip "$BUNDLE"/nsis/*-setup.nsis.zip.sig; do
      name=$(basename "$src")
      stage "$src" "${name//x64-setup/amd64}"
    done
    ;;
  *)
    echo "unknown matrix label: $LABEL" >&2
    exit 1
    ;;
esac

echo "--- staged in $STAGE ---"
ls -la "$STAGE"
