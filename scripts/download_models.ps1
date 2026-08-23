# Download all-MiniLM-L6-v2 Candle embedding model assets for Broomed (PowerShell)
param(
    [string]$DestDir = "src-tauri/resources/models/all-MiniLM-L6-v2"
)

$ErrorActionPreference = "Stop"
$BaseUrl = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main"

if (-not (Test-Path $DestDir)) {
    New-Item -ItemType Directory -Path $DestDir -Force | Out-Null
}

Write-Host "Downloading all-MiniLM-L6-v2 model assets to $DestDir..." -ForegroundColor Cyan

$files = @(
    "model.safetensors",
    "config.json",
    "tokenizer.json"
)

foreach ($file in $files) {
    $dest = Join-Path $DestDir $file
    if (Test-Path $dest) {
        Write-Host "  [skip] $file already exists" -ForegroundColor Gray
    } else {
        Write-Host "  [download] $file..." -ForegroundColor Yellow
        $url = "$BaseUrl/$file"
        Invoke-WebRequest -Uri $url -OutFile $dest
    }
}

Write-Host "Done! Model assets downloaded to $DestDir" -ForegroundColor Green
