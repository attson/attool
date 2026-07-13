#!/usr/bin/env bash
# Copy Tauri bundle outputs into a staging dir + pack updater archives.
#
# Usage: stage-bundles.sh <rust-target-triple> <matrix-label> <staging-dir>
#
# Products per platform:
#   macos-arm64   -> AT.Tool_<v>_arm64.dmg (user installer)
#                    AT.Tool_<v>_arm64.app.tar.gz (updater archive; contains "AT Tool.app")
#   macos-x64     -> AT.Tool_<v>_amd64.dmg + AT.Tool_<v>_amd64.app.tar.gz
#   linux-x64     -> AT.Tool_<v>_amd64.deb + AT.Tool_<v>_amd64.tar.gz (contains raw binary)
#   linux-arm64   -> AT.Tool_<v>_arm64.deb + AT.Tool_<v>_arm64.tar.gz
#   windows-x64   -> AT.Tool_<v>_amd64.exe (NSIS installer) + AT.Tool_<v>_amd64.exe.zip
#                    (updater archive; contains "AT Tool.exe")
#
# GitHub renames spaces to dots when uploading assets, so we produce dotted
# names here to match what expected_asset_name() computes on the client.

set -euo pipefail
shopt -s nullglob

TARGET="$1"
LABEL="$2"
STAGE="$3"

mkdir -p "$STAGE"
BUNDLE="src-tauri/target/$TARGET/release/bundle"
TARGET_BIN="src-tauri/target/$TARGET/release"

# Extract version from src-tauri/tauri.conf.json for canonical filename stem
VERSION=$(node -e "process.stdout.write(require('./src-tauri/tauri.conf.json').version)")
STEM="AT.Tool_${VERSION}"

case "$LABEL" in
  macos-arm64|macos-x64)
    if [ "$LABEL" = "macos-arm64" ]; then ARCH="arm64"; else ARCH="amd64"; fi
    # dmg
    for src in "$BUNDLE"/dmg/*.dmg; do
      cp "$src" "$STAGE/${STEM}_${ARCH}.dmg"
    done
    # pack app bundle into updater archive
    APP_DIR="$BUNDLE/macos"
    if [ ! -d "$APP_DIR/AT Tool.app" ]; then
      echo "ERROR: $APP_DIR/AT Tool.app not found" >&2
      ls -la "$APP_DIR" >&2
      exit 1
    fi
    (cd "$APP_DIR" && tar -czf "$STAGE/${STEM}_${ARCH}.app.tar.gz" "AT Tool.app")
    ;;

  linux-x64|linux-arm64)
    if [ "$LABEL" = "linux-x64" ]; then ARCH="amd64"; else ARCH="arm64"; fi
    # deb (usually named at-tool_<v>_amd64.deb; pick any)
    for src in "$BUNDLE"/deb/*.deb; do
      cp "$src" "$STAGE/${STEM}_${ARCH}.deb"
    done
    # find the raw binary (Cargo name in Cargo.toml is "attool")
    BIN="$TARGET_BIN/attool"
    if [ ! -f "$BIN" ]; then
      echo "ERROR: $BIN not found" >&2
      ls -la "$TARGET_BIN" >&2
      exit 1
    fi
    # tar the raw binary — install-linux.sh will find & mv the single file
    (cd "$TARGET_BIN" && tar -czf "$STAGE/${STEM}_${ARCH}.tar.gz" attool)
    ;;

  windows-x64)
    # NSIS installer (user download)
    for src in "$BUNDLE"/nsis/*-setup.exe; do
      cp "$src" "$STAGE/${STEM}_amd64.exe"
    done
    # pack the raw exe for updater
    EXE_SRC="$TARGET_BIN/AT Tool.exe"
    if [ ! -f "$EXE_SRC" ]; then
      # fallback: some Tauri versions ship the exe under a different name in target/release
      alt=$(ls "$TARGET_BIN"/*.exe 2>/dev/null | grep -v setup | head -1 || true)
      if [ -n "$alt" ]; then
        EXE_SRC="$alt"
      else
        echo "ERROR: no exe found in $TARGET_BIN" >&2
        ls -la "$TARGET_BIN" >&2
        exit 1
      fi
    fi
    # zip on windows runner may not be present; try 7z (default on GH windows runners)
    if command -v 7z >/dev/null 2>&1; then
      7z a "$STAGE/${STEM}_amd64.exe.zip" "$EXE_SRC" > /dev/null
    else
      (cd "$(dirname "$EXE_SRC")" && zip -q "$STAGE/${STEM}_amd64.exe.zip" "$(basename "$EXE_SRC")")
    fi
    ;;

  *)
    echo "unknown matrix label: $LABEL" >&2
    exit 1
    ;;
esac

echo "--- staged in $STAGE ---"
ls -la "$STAGE"
