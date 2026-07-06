#!/usr/bin/env sh
set -eu

# 下载当前默认 fastembed 模型到仓库内 model/fastembed，供本地配置直接复用。
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
CACHE_DIR="${SCRIPT_DIR}/fastembed"
MODEL_DIR_NAME="models--Qdrant--all-MiniLM-L6-v2-onnx"
SNAPSHOT="5f1b8cd78bc4fb444dd171e59b18f3a3af89a079"
SNAPSHOT_DIR="${CACHE_DIR}/${MODEL_DIR_NAME}/snapshots/${SNAPSHOT}"
REF_DIR="${CACHE_DIR}/${MODEL_DIR_NAME}/refs"
BASE_URL="https://huggingface.co/Qdrant/all-MiniLM-L6-v2-onnx/resolve/main"

download_if_missing() {
    target="$1"
    url="$2"
    if [ -f "$target" ]; then
        printf 'skip %s\n' "$target"
        return 0
    fi

    mkdir -p "$(dirname "$target")"
    printf 'download %s\n' "$url"
    curl -fL --retry 3 --retry-delay 2 -o "$target" "$url"
}

mkdir -p "$REF_DIR" "$SNAPSHOT_DIR"
# hf-hub 0.3.2 读取 refs/main 时不会 trim，必须写入无换行 commit hash。
printf %s "$SNAPSHOT" > "${REF_DIR}/main"

download_if_missing "${SNAPSHOT_DIR}/model.onnx" "${BASE_URL}/model.onnx"
download_if_missing "${SNAPSHOT_DIR}/tokenizer.json" "${BASE_URL}/tokenizer.json"
download_if_missing "${SNAPSHOT_DIR}/config.json" "${BASE_URL}/config.json"
download_if_missing "${SNAPSHOT_DIR}/special_tokens_map.json" "${BASE_URL}/special_tokens_map.json"
download_if_missing "${SNAPSHOT_DIR}/tokenizer_config.json" "${BASE_URL}/tokenizer_config.json"

printf 'model cache ready: %s\n' "$SNAPSHOT_DIR"
