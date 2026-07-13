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

# 找出解压出来的可执行文件（应该只有一个）
new_bin=""
for candidate in "$tmp"/*; do
  if [ -f "$candidate" ]; then
    new_bin="$candidate"
    break
  fi
done
if [ -z "$new_bin" ]; then
  echo "archive did not contain an executable" >&2
  exit 1
fi
chmod +x "$new_bin"

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
