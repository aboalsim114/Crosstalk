#!/usr/bin/env bash
# Test build for Windows x64 from Linux with MinGW-w64 (gcc), used when no
# Windows build machine (MSVC) is at hand. Produces the installer (design
# "Crosstalk Installer", src-installer) and a portable folder with the
# command-line tools:
#   dist-test/Crosstalk-<commit>-setup.exe
#   dist-test/crosstalk-<commit>-windows-x64-gcc-test/
#
# Requirements (Ubuntu 24.04): apt install mingw-w64 nsis libclang-dev cmake glslc libvulkan-dev
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

# Whisper on the GPU (Vulkan, feature "gpu"). ggml takes vkGetInstanceProcAddr
# from the Vulkan loader: our shim provides it and loads vulkan-1.dll at run
# time, so the app still starts (on the CPU) without a Vulkan driver. CMake
# finds it as the SDK's vulkan-1 library; headers and glslc come from Ubuntu.
vk="$root/src-tauri/target/vulkan-shim"
mkdir -p "$vk"
x86_64-w64-mingw32-gcc-posix -O2 -Wall -c scripts/cross/vulkan_shim.c -o "$vk/vulkan_shim.o"
rm -f "$vk/libvulkan.a" && x86_64-w64-mingw32-ar rcs "$vk/libvulkan.a" "$vk/vulkan_shim.o"
# FindVulkan looks in include/, lib/ and bin/ on a Linux host. Only the Vulkan
# headers: all of /usr/include would hand glibc's to MinGW.
mkdir -p "$vk/include" "$vk/lib"
ln -sfn /usr/include/vulkan "$vk/include/vulkan"
ln -sfn /usr/include/vk_video "$vk/include/vk_video"
cp "$vk/libvulkan.a" "$vk/lib/libvulkan-1.a"
ln -sfn /usr/bin "$vk/bin"
export VULKAN_SDK="$vk"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="$CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS -L native=$vk"

# MinGW names ggml's archives without the "lib" prefix rustc expects.
fix_archives() {
  for d in src-tauri/target/$target/release/build/whisper-rs-sys-*/out; do
    [ -d "$d" ] || continue
    find "$d" -name 'ggml*.a' | while read -r f; do cp --update=none "$f" "$d/lib$(basename "$f")"; done
  done
}
build() { (cd src-tauri && cargo build --release --locked --target $target --features gpu "$@"); }

# First pass builds whisper.cpp (and fails at link), second pass links.
build --example asr_bench --example audio_probe --example translate || { fix_archives; build --example asr_bench --example audio_probe --example translate; }
fix_archives
# The app through the Tauri CLI (builds and embeds its UI). MinGW builds need
# libstdc++ and WebView2Loader.dll next to the exe.
gcc_dir=$(dirname "$(x86_64-w64-mingw32-g++-posix -print-libgcc-file-name)")
rel="src-tauri/target/$target/release"
npx tauri build --target $target --no-bundle --features gpu
version=$(node -p "require('./src-tauri/tauri.conf.json').version")

# The installer (src-installer, "Crosstalk Installer" design) embeds those files,
# built in the app's target directory so shared dependencies compile once.
payload=$(mktemp -d)
cp "$rel/crosstalk.exe" "$rel/WebView2Loader.dll" "$payload/"
cp "$gcc_dir/libstdc++-6.dll" "$gcc_dir/libgcc_s_seh-1.dll" /usr/x86_64-w64-mingw32/lib/libwinpthread-1.dll "$payload/"
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
cp "$rel/examples/translate.exe" "$out/crosstalk-translate.exe"
cp "$rel/WebView2Loader.dll" "$out/"
cp "$gcc_dir/libstdc++-6.dll" "$gcc_dir/libgcc_s_seh-1.dll" /usr/x86_64-w64-mingw32/lib/libwinpthread-1.dll "$out/"
cp scripts/download-models.ps1 scripts/fetch-asr-corpus.py "$out/"
sed "s/@COMMIT@/$commit/g; s/\$/\r/" scripts/cross/LISEZMOI-TEST.txt > "$out/LISEZMOI-TEST.txt"
(cd "$out" && sha256sum ./*.exe ./*.dll > SHA256SUMS.txt)
(cd dist-test && rm -f "$(basename "$out").zip" && zip -qr "$(basename "$out").zip" "$(basename "$out")")
echo "paquet : $out.zip"
