#!/usr/bin/env sh
set -eu

# 下载仓库当前验证通过的 ONNX Runtime shared library 到 runtime/onnxruntime/。
# 目前只覆盖 Linux x86_64 / aarch64；其他平台仍需手动准备匹配的 shared library。

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
RUNTIME_ROOT="${SCRIPT_DIR}/onnxruntime"
DOWNLOAD_DIR="${RUNTIME_ROOT}/downloads"
RELEASE_TAG="${ORT_RELEASE_TAG:-v1.24.4}"
VERSION="${RELEASE_TAG#v}"
UNAME_S=$(uname -s)
UNAME_M=$(uname -m)

case "$UNAME_S" in
  Linux) ;;
  *)
    printf 'unsupported OS for this helper: %s\n' "$UNAME_S" >&2
    exit 1
    ;;
esac

case "$UNAME_M" in
  x86_64|amd64)
    ASSET_NAME="onnxruntime-linux-x64-glibc2_17-Release-${VERSION}.zip"
    ;;
  aarch64|arm64)
    ASSET_NAME="onnxruntime-linux-aarch64-glibc2_17-Release-${VERSION}.zip"
    ;;
  *)
    printf 'unsupported CPU architecture for this helper: %s\n' "$UNAME_M" >&2
    exit 1
    ;;
esac

ASSET_DIR_NAME=${ASSET_NAME%.zip}
TAG_DIR="${RUNTIME_ROOT}/${RELEASE_TAG}"
EXTRACT_DIR="${TAG_DIR}/${ASSET_DIR_NAME}"
ARCHIVE_PATH="${DOWNLOAD_DIR}/${ASSET_NAME}"
CURRENT_LINK="${RUNTIME_ROOT}/current"
ASSET_URL="https://github.com/csukuangfj/onnxruntime-libs/releases/download/${RELEASE_TAG}/${ASSET_NAME}"

mkdir -p "$DOWNLOAD_DIR" "$TAG_DIR"

if [ ! -f "$ARCHIVE_PATH" ]; then
  printf 'download %s\n' "$ASSET_URL"
  curl -fL --retry 3 --retry-delay 2 -o "$ARCHIVE_PATH" "$ASSET_URL"
else
  printf 'skip archive %s\n' "$ARCHIVE_PATH"
fi

if [ ! -d "$EXTRACT_DIR" ]; then
  printf 'extract %s\n' "$ARCHIVE_PATH"
  python3 - "$ARCHIVE_PATH" "$TAG_DIR" <<'PY'
import pathlib
import sys
import zipfile

archive = pathlib.Path(sys.argv[1])
out_dir = pathlib.Path(sys.argv[2])
with zipfile.ZipFile(archive) as zf:
    zf.extractall(out_dir)
PY
else
  printf 'skip extract %s\n' "$EXTRACT_DIR"
fi

ln -sfn "$EXTRACT_DIR" "$CURRENT_LINK"
LIB_DIR="${CURRENT_LINK}/lib"
LIB_PATH="${LIB_DIR}/libonnxruntime.so"

if [ ! -f "$LIB_PATH" ]; then
  printf 'expected shared library not found: %s\n' "$LIB_PATH" >&2
  exit 1
fi

printf 'onnxruntime ready: %s\n' "$LIB_PATH"
printf 'export ORT_DYLIB_PATH="%s"\n' "$LIB_PATH"
printf 'export LD_LIBRARY_PATH="%s:${LD_LIBRARY_PATH:-}"\n' "$LIB_DIR"
