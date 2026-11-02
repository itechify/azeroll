#!/usr/bin/env bash
# Cross-compile the Windows x86_64 Azeroll player build from Linux and package it as
# `$OUT/azeroll-win.zip` (azeroll.exe, Play Azeroll.bat, README.txt, an empty Data/). No game data
# goes in the zip: the player puts a 1.12.1 client's MPQs in Data/.
#
# Requires: rustup (pinned toolchain), clang-cl, zip. Installs the msvc target and cargo-xwin on
# first use; cargo-xwin downloads the MSVC CRT and Windows SDK into ~/.cache/cargo-xwin.
# The build uses its own target dir (target-win) and a static CRT, so the exe needs no
# redistributable DLLs.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${OUT:-$HOME}"
PKG="$OUT/azeroll-win"
export PATH="$HOME/.cargo/bin:$PATH"

rustup target add x86_64-pc-windows-msvc >/dev/null
command -v cargo-xwin >/dev/null || cargo install cargo-xwin --locked

# There is no lld-link on the build host: a shim hands the MSVC-flavoured link to rustup's
# bundled rust-lld.
SHIM="$(mktemp -d)"
trap 'rm -rf "$SHIM"' EXIT
LLD="$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld"
cat >"$SHIM/lld-link" <<EOF
#!/bin/sh
[ "\$1" = "-flavor" ] && shift 2
exec "$LLD" -flavor link "\$@"
EOF
chmod +x "$SHIM/lld-link"

cd "$ROOT"
# No `dev` feature: it bakes this machine's source path into the binary.
PATH="$SHIM:$PATH" CARGO_TARGET_DIR="$ROOT/target-win" RUSTFLAGS="-C target-feature=+crt-static" \
    cargo xwin build --profile play -p azeroll --no-default-features \
    --target x86_64-pc-windows-msvc --cross-compiler clang-cl

mkdir -p "$PKG/Data"
cp "$ROOT/target-win/x86_64-pc-windows-msvc/play/azeroll.exe" "$PKG/"
printf '%s\r\n' \
    '@echo off' \
    'cd /d "%~dp0"' \
    'set "WOW_DATA=%~dp0Data"' \
    'echo Azeroll: loading Elwynn Forest. The world takes about 2 minutes on first launch.' \
    'azeroll.exe' \
    'if errorlevel 1 pause' >"$PKG/Play Azeroll.bat"
printf '%s\r\n' \
    'Azeroll - Everything is Bind on Pickup.' \
    '' \
    'Put the WoW 1.12.1 client'"'"'s Data\*.MPQ files in the Data folder next to this file,' \
    'then run "Play Azeroll.bat".' \
    '' \
    'W/S roll, A/D turn, Shift dash, Space quick turn, mouse drag/wheel camera,' \
    'Esc pause, R restart, Enter to roll again at the results.' >"$PKG/README.txt"
rm -f "$OUT/azeroll-win.zip"
(cd "$OUT" && zip -q -r -9 azeroll-win.zip azeroll-win)
echo "packaged $OUT/azeroll-win.zip"
echo "  send it: tailscale file cp $OUT/azeroll-win.zip jjd-pc:"
