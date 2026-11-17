#!/bin/sh
# Cross-compile the Windows exe from Linux (needs mingw-w64 and the windows-gnu std:
#   rustup target add x86_64-pc-windows-gnu ; apt install gcc-mingw-w64-x86-64)
set -e
cd "$(dirname "$0")/.."
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
  cargo build --release --target x86_64-pc-windows-gnu "$@"
echo "-> target/x86_64-pc-windows-gnu/release/openswitch.exe"
