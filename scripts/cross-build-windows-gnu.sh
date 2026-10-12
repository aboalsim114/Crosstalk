#!/usr/bin/env bash
# Test build for Windows x64 from Linux with MinGW-w64 (gcc), used when no
# Windows build machine (MSVC) is at hand. Produces a portable folder, not an installer:
#   dist-test/crosstalk-<commit>-windows-x64-gcc-test/
#
# Requirements (Ubuntu 24.04): apt install mingw-w64 nsis libclang-dev cmake
#                              rustup target add x86_64-pc-windows-gnu
# The official release stays the MSVC build (scripts/validate.ps1); the compiler only
# changes after a measured comparison (docs/testing/speech-recognition.md).
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
target=x86_64-pc-windows-gnu
commit=$(git rev-parse --short HEAD)
[ -z "$(git status --porcelain)" ] || echo "attention : arbre de travail modifié, le paquet ne correspond pas exactement à $commit" >&2

export LIBCLANG_PATH=${LIBCLANG_PATH:-/usr/lib/llvm-18/lib}
export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc-posix
export CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++-posix
export AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc-posix
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C link-args=-static"
# MinGW-w64 11 lacks the thread power-throttling declarations used by ggml
# (opt-out of Windows 11 EcoQoS): provide them instead of disabling it.
compat="$root/scripts/cross/mingw_compat.h"
export CMAKE_C_FLAGS="-include $compat" CMAKE_CXX_FLAGS="-include $compat"

# MinGW names ggml's archives without the "lib" prefix rustc expects.
fix_archives() {
  for d in src-tauri/target/$target/release/build/whisper-rs-sys-*/out; do
    [ -d "$d" ] || continue
    find "$d" -name 'ggml*.a' | while read -r f; do cp --update=none "$f" "$d/lib$(basename "$f")"; done
  done
}
build() { (cd src-tauri && cargo build --release --locked --target $target "$@"); }

# First pass builds whisper.cpp (and fails at link), second pass links.
build --example asr_bench --example audio_probe || { fix_archives; build --example asr_bench --example audio_probe; }
fix_archives
# The app itself through the Tauri CLI (builds and embeds the UI). No bundle:
# the NSIS installer would download the WebView2 bootstrapper.
npx tauri build --target $target --no-bundle

out="dist-test/crosstalk-$commit-windows-x64-gcc-test"
rel="src-tauri/target/$target/release"
rm -rf "$out" && mkdir -p "$out"
cp "$rel/crosstalk.exe" "$out/"
cp "$rel/examples/asr_bench.exe" "$out/crosstalk-asr-bench.exe"
cp "$rel/examples/audio_probe.exe" "$out/crosstalk-audio-probe.exe"
cp "$rel/WebView2Loader.dll" "$out/"
gcc_dir=$(dirname "$(x86_64-w64-mingw32-g++-posix -print-libgcc-file-name)")
cp "$gcc_dir/libstdc++-6.dll" "$gcc_dir/libgcc_s_seh-1.dll" /usr/x86_64-w64-mingw32/lib/libwinpthread-1.dll "$out/"
cp scripts/download-models.ps1 scripts/fetch-asr-corpus.py "$out/"
sed "s/@COMMIT@/$commit/g; s/\$/\r/" scripts/cross/LISEZMOI-TEST.txt > "$out/LISEZMOI-TEST.txt"
(cd "$out" && sha256sum ./*.exe ./*.dll > SHA256SUMS.txt)
(cd dist-test && rm -f "$(basename "$out").zip" && zip -qr "$(basename "$out").zip" "$(basename "$out")")
echo "paquet : $out.zip"
