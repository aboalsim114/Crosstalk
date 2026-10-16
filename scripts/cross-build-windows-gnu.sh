#!/usr/bin/env bash
# Test build for Windows x64 from Linux with MinGW-w64 (gcc), used when no
# Windows build machine (MSVC) is at hand. Produces the installer (design
# "Crosstalk Installer", src-installer) and a portable folder with the
# command-line tools:
#   dist-test/Crosstalk-<commit>-setup.exe
#   dist-test/crosstalk-<commit>-windows-x64-gcc-test/
#
# Requirements (Ubuntu 24.04): apt install mingw-w64 nsis libclang-dev
#                              rustup target add x86_64-pc-windows-gnu
# Speech recognition runs in the prebuilt sherpa-onnx / ONNX Runtime DLLs of
# the official release (scripts/fetch-sherpa-onnx.sh windows): the Rust code
# links to their C API and the DLLs ship next to crosstalk.exe.
# The official release stays the MSVC build (scripts/validate.ps1).
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
target=x86_64-pc-windows-gnu
commit=$(git rev-parse --short HEAD)
[ -z "$(git status --porcelain)" ] || echo "attention : arbre de travail modifié, le paquet ne correspond pas exactement à $commit" >&2

export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc-posix
export CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++-posix
export AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc-posix
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C link-args=-static"
eval "$(scripts/fetch-sherpa-onnx.sh windows)"
# The DLLs the recogniser loads (C API, ONNX Runtime): the "MT" builds, with
# the C runtime inside, so nothing else is needed.
asr_dlls="sherpa-onnx-c-api.dll onnxruntime.dll onnxruntime_providers_shared.dll"

build() { (cd src-tauri && cargo build --release --locked --target $target "$@"); }

build --example asr_bench --example audio_probe
# The app through the Tauri CLI (builds and embeds its UI). It needs
# WebView2Loader.dll and the recogniser's DLLs next to the exe.
rel="src-tauri/target/$target/release"
npx tauri build --target $target --no-bundle
version=$(node -p "require('./src-tauri/tauri.conf.json').version")

# The installer (src-installer, "Crosstalk Installer" design) embeds those files,
# built in the app's target directory so shared dependencies compile once.
payload=$(mktemp -d)
cp "$rel/crosstalk.exe" "$rel/WebView2Loader.dll" "$payload/"
for d in $asr_dlls; do cp "$SHERPA_ONNX_LIB_DIR/$d" "$payload/"; done
(cd src-installer && CROSSTALK_PAYLOAD_DIR="$payload" CARGO_TARGET_DIR="$root/src-tauri/target" \
  npx tauri build --target $target --no-bundle -- --locked)

# Crosstalk-setup.exe: NSIS shell that installs WebView2 if needed, runs the
# installer and writes the uninstaller (scripts/cross/setup-wrapper.nsi).
files_nsh=$(mktemp --suffix=.nsh)
for f in "$payload"/*; do echo "Delete \"\$INSTDIR\\$(basename "$f")\"" >> "$files_nsh"; done
app_kb=$(( $(du -sb "$payload" | cut -f1) / 1024 ))
mkdir -p dist-test
makensis -V2 -INPUTCHARSET UTF8 \
  -DVERSION="$version" -DOUTFILE="$root/dist-test/Crosstalk-$commit-setup.exe" \
  -DSETUP_EXE="$root/$rel/crosstalk-setup.exe" -DLOADER="$root/$rel/WebView2Loader.dll" \
  -DICON="$root/src-tauri/icons/icon.ico" -DFILES_NSH="$files_nsh" -DAPP_KB="$app_kb" \
  scripts/cross/setup-wrapper.nsi
rm -rf "$payload" "$files_nsh"
echo "installateur : dist-test/Crosstalk-$commit-setup.exe"

out="dist-test/crosstalk-$commit-windows-x64-gcc-test"
rm -rf "$out" && mkdir -p "$out"
cp "$rel/crosstalk.exe" "$out/"
cp "$rel/examples/asr_bench.exe" "$out/crosstalk-asr-bench.exe"
cp "$rel/examples/audio_probe.exe" "$out/crosstalk-audio-probe.exe"
cp "$rel/WebView2Loader.dll" "$out/"
for d in $asr_dlls; do cp "$SHERPA_ONNX_LIB_DIR/$d" "$out/"; done
cp scripts/download-models.ps1 scripts/fetch-asr-corpus.py "$out/"
sed "s/@COMMIT@/$commit/g; s/\$/\r/" scripts/cross/LISEZMOI-TEST.txt > "$out/LISEZMOI-TEST.txt"
(cd "$out" && sha256sum ./*.exe ./*.dll > SHA256SUMS.txt)
(cd dist-test && rm -f "$(basename "$out").zip" && zip -qr "$(basename "$out").zip" "$(basename "$out")")
echo "paquet : $out.zip"
