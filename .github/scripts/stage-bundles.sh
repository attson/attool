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
  # GitHub release uploads replace spaces with dots in stored asset names.
  # Pre-rename here so latest.json URLs match the actual asset filenames.
  newname="${newname// /.}"
  echo "stage: $(basename "$src") -> $newname"
  cp "$src" "$STAGE/$newname"
}

mac_app_name() {
  local name="$1"
  local arch="$2"
  local sig=""
  local base

  if [[ "$name" == *.app.tar.gz.sig ]]; then
    sig=".sig"
    base="${name%.app.tar.gz.sig}"
  else
    base="${name%.app.tar.gz}"
  fi

  base="${base//aarch64/arm64}"
  base="${base//x86_64/amd64}"
  base="${base//x64/amd64}"
  if [[ "$base" != *_"$arch" ]]; then
    base="${base}_${arch}"
  fi

  printf '%s.app.tar.gz%s\n' "$base" "$sig"
}

case "$LABEL" in
  macos-arm64)
    for src in "$BUNDLE"/dmg/*.dmg; do
      name=$(basename "$src")
      stage "$src" "${name//aarch64/arm64}"
    done
    for src in "$BUNDLE"/macos/*.app.tar.gz "$BUNDLE"/macos/*.app.tar.gz.sig; do
      name=$(basename "$src")
      stage "$src" "$(mac_app_name "$name" arm64)"
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
      stage "$src" "$(mac_app_name "$name" amd64)"
    done
    ;;
  linux-x64|linux-arm64)
    for src in "$BUNDLE"/deb/*.deb "$BUNDLE"/deb/*.deb.sig; do
      stage "$src" "$(basename "$src")"
    done
    ;;
  windows-x64)
    # User-facing installer
    for src in "$BUNDLE"/nsis/*-setup.exe "$BUNDLE"/nsis/*-setup.exe.sig; do
      name=$(basename "$src")
      newname="${name//x64-setup.exe.sig/amd64.exe.sig}"
      newname="${newname//x64-setup.exe/amd64.exe}"
      stage "$src" "$newname"
    done
    # Keep compatibility if a future Tauri release emits a zipped NSIS updater.
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
