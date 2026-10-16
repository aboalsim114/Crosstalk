#!/usr/bin/env bash
# Downloads the prebuilt sherpa-onnx libraries (C API + ONNX Runtime) used by
# the speech recogniser, for Linux (tests) and Windows x64 (the app), into
# src-tauri/target/sherpa-onnx/<platform>/lib, and prints the line to export.
# The Rust build reads SHERPA_ONNX_LIB_DIR; without it, it downloads them itself.
#   source <(scripts/fetch-sherpa-onnx.sh linux)     # or: windows
set -euo pipefail
version=1.13.8
platform=${1:-linux}
case "$platform" in
  linux) name="sherpa-onnx-v$version-linux-x64-shared-lib" ;;
  windows) name="sherpa-onnx-v$version-win-x64-shared-MT-Release-lib" ;;
  *) echo "plateforme inconnue : $platform (linux ou windows)" >&2; exit 1 ;;
esac
root=$(cd "$(dirname "$0")/.." && pwd)
dest="$root/src-tauri/target/sherpa-onnx/$platform"
if [ ! -d "$dest/$name/lib" ]; then
  mkdir -p "$dest"
  curl -fsSL "https://github.com/k2-fsa/sherpa-onnx/releases/download/v$version/$name.tar.bz2" | tar -xj -C "$dest"
fi
echo "export SHERPA_ONNX_LIB_DIR=$dest/$name/lib"
