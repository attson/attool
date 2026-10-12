#!/bin/bash
# attool auto-update installer for Linux.
# Args: <pid> <src-archive> <dst-binary>
set -e
pid=$1
src=$2
dst=$3

log_dir="${HOME}/.local/share/attool"
mkdir -p "$log_dir"
log="${log_dir}/update-${pid}.log"
exec 2>>"$log"

echo "[$(date)] waiting for pid $pid to exit" >&2
for i in {1..60}; do
  kill -0 "$pid" 2>/dev/null || break
  sleep 0.5
done

tmp=$(mktemp -d)
tar -xzf "$src" -C "$tmp"

# 新版归档固定为 attool；兼容只含单个二进制的旧归档。
new_bin="$tmp/attool"
if [ ! -f "$new_bin" ]; then
  new_bin=""
  for candidate in "$tmp"/*; do
    if [ -f "$candidate" ] && [ -x "$candidate" ]; then
      new_bin="$candidate"
      break
    fi
  done
fi
if [ -z "$new_bin" ]; then
  echo "archive did not contain an executable" >&2
  exit 1
fi
chmod +x "$new_bin"

layer_shell_src="$tmp/libgtk-layer-shell.so.0"
if [ -f "$layer_shell_src" ]; then
  data_home="${XDG_DATA_HOME:-${HOME}/.local/share}"
  layer_shell_dir="$data_home/attool/lib"
  mkdir -p "$layer_shell_dir"
  cp "$layer_shell_src" "$layer_shell_dir/libgtk-layer-shell.so.0"
fi

echo "[$(date)] replacing $dst with $new_bin" >&2
if ! mv "$new_bin" "$dst" 2>/dev/null; then
  if ! command -v pkexec >/dev/null 2>&1; then
    echo "could not replace $dst and pkexec is not available" >&2
    exit 1
  fi
  pkexec /bin/sh -c 'mv "$1" "$2" && chmod +x "$2"' sh "$new_bin" "$dst"
fi

setsid "$dst" >/dev/null 2>&1 < /dev/null &

rm -f "$src"
rm -rf "$tmp"
echo "[$(date)] update complete" >&2
