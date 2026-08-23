#!/usr/bin/env bash
# Download all-MiniLM-L6-v2 Candle embedding model assets for Broomed
set -euo pipefail

DEST_DIR="${1:-src-tauri/resources/models/all-MiniLM-L6-v2}"
BASE_URL="https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main"

mkdir -p "$DEST_DIR"

echo "Downloading all-MiniLM-L6-v2 model assets to $DEST_DIR..."

FILES=(
  "model.safetensors"
  "config.json"
  "tokenizer.json"
)

for file in "${FILES[@]}"; do
  dest="$DEST_DIR/$file"
  if [ -f "$dest" ]; then
    echo "  [skip] $file already exists"
  else
    echo "  [download] $file..."
    curl -fSL "$BASE_URL/$file" -o "$dest"
  fi
done

echo "Done! Model assets downloaded to $DEST_DIR"
