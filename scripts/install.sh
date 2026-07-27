#!/usr/bin/env sh
set -eu

target="${WTW_INSTALL_DIR:-$HOME/.local/bin}"
triple="${WTW_TARGET:-x86_64-unknown-linux-gnu}"
archive="$(mktemp)"
work="$(mktemp -d)"
trap 'rm -f "$archive"; rm -rf "$work"' EXIT

mkdir -p "$target"
curl -fsSL "https://github.com/lucasrgt/why-this-way/releases/latest/download/wtw-$triple.zip" -o "$archive"
unzip -q "$archive" -d "$work"
cp "$work/wtw-$triple/wtw" "$target/wtw"
chmod +x "$target/wtw"
printf 'Installed wtw to %s/wtw\n' "$target"
