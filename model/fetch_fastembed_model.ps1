$ErrorActionPreference = 'Stop'

# 下载当前默认 fastembed 模型到仓库内 model/fastembed，供 Windows release 直接复用。
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$CacheDir = Join-Path $ScriptDir 'fastembed'
$ModelDirName = 'models--Qdrant--all-MiniLM-L6-v2-onnx'
$Snapshot = '5f1b8cd78bc4fb444dd171e59b18f3a3af89a079'
$SnapshotDir = Join-Path $CacheDir "$ModelDirName/snapshots/$Snapshot"
$RefDir = Join-Path $CacheDir "$ModelDirName/refs"
$BaseUrl = 'https://huggingface.co/Qdrant/all-MiniLM-L6-v2-onnx/resolve/main'

function Download-IfMissing {
    param(
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$Url
    )

    if (Test-Path $Target) {
        Write-Host "skip $Target"
        return
    }

    $Parent = Split-Path -Parent $Target
    New-Item -ItemType Directory -Force -Path $Parent | Out-Null
    Write-Host "download $Url"
    Invoke-WebRequest -Uri $Url -OutFile $Target
}

New-Item -ItemType Directory -Force -Path $RefDir | Out-Null
New-Item -ItemType Directory -Force -Path $SnapshotDir | Out-Null

# hf-hub 0.3.2 读取 refs/main 时不会 trim，必须写入无换行 commit hash。
[System.IO.File]::WriteAllText((Join-Path $RefDir 'main'), $Snapshot)

Download-IfMissing -Target (Join-Path $SnapshotDir 'model.onnx') -Url "$BaseUrl/model.onnx"
Download-IfMissing -Target (Join-Path $SnapshotDir 'tokenizer.json') -Url "$BaseUrl/tokenizer.json"
Download-IfMissing -Target (Join-Path $SnapshotDir 'config.json') -Url "$BaseUrl/config.json"
Download-IfMissing -Target (Join-Path $SnapshotDir 'special_tokens_map.json') -Url "$BaseUrl/special_tokens_map.json"
Download-IfMissing -Target (Join-Path $SnapshotDir 'tokenizer_config.json') -Url "$BaseUrl/tokenizer_config.json"

Write-Host "model cache ready: $SnapshotDir"
