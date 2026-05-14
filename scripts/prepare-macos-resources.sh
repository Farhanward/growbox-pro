#!/usr/bin/env bash
# prepare-macos-resources.sh
#
# Run once on a Mac before `npm run tauri dev` or `npm run tauri build`.
# Downloads macOS-native sidecars (llama.cpp, Node.js, FFmpeg) into
# src-tauri/resources/ so the Tauri bundle can find them at runtime.
#
# Usage:
#   chmod +x scripts/prepare-macos-resources.sh
#   ./scripts/prepare-macos-resources.sh
#
# Environment overrides:
#   NODE_LTS_RELEASE   – Node.js version to bundle  (default: v22.15.0)
#   SKIP_LLAMA_BUILD   – set to "1" to skip building llama.cpp from source

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LLAMA_DEST="$REPO_ROOT/src-tauri/resources/llama"
NODE_DEST="$REPO_ROOT/src-tauri/resources/node"
NODE_LTS_RELEASE="${NODE_LTS_RELEASE:-v22.15.0}"

# Detect architecture
ARCH="$(uname -m)"
if [ "$ARCH" = "arm64" ]; then
  NODE_PLATFORM="darwin-arm64"
else
  NODE_PLATFORM="darwin-x64"
fi

echo "=== GrowBox Pro — macOS resource preparation ==="
echo "Architecture : $ARCH"
echo "Node version : $NODE_LTS_RELEASE ($NODE_PLATFORM)"
echo ""

mkdir -p "$LLAMA_DEST" "$NODE_DEST"

# ── 1. Remove Windows-only binaries ─────────────────────────────────────────
echo "→ Removing Windows binaries (.exe / .dll)…"
rm -f "$LLAMA_DEST"/*.exe "$LLAMA_DEST"/*.dll "$NODE_DEST"/node.exe

# ── 2. Node.js macOS binary ──────────────────────────────────────────────────
if [ -f "$NODE_DEST/node" ]; then
  CURRENT=$("$NODE_DEST/node" --version 2>/dev/null || echo "unknown")
  echo "→ Node.js already bundled ($CURRENT) — skipping download."
else
  echo "→ Downloading Node.js ${NODE_LTS_RELEASE} (${NODE_PLATFORM})…"
  NODE_TARBALL="node-${NODE_LTS_RELEASE}-${NODE_PLATFORM}.tar.gz"
  NODE_URL="https://nodejs.org/dist/${NODE_LTS_RELEASE}/${NODE_TARBALL}"
  curl -fsSL "$NODE_URL" -o /tmp/node.tar.gz
  tar -xzf /tmp/node.tar.gz \
    --strip-components=2 \
    -C /tmp \
    "node-${NODE_LTS_RELEASE}-${NODE_PLATFORM}/bin/node"
  mv /tmp/node "$NODE_DEST/node"
  chmod +x "$NODE_DEST/node"
  echo "   Bundled: $("$NODE_DEST/node" --version)"
fi

# ── 3. FFmpeg static binary ──────────────────────────────────────────────────
if [ -f "$LLAMA_DEST/ffmpeg" ]; then
  echo "→ FFmpeg already bundled — skipping download."
else
  echo "→ Downloading static FFmpeg from evermeet.cx…"
  curl -fsSL "https://evermeet.cx/ffmpeg/getrelease/ffmpeg/zip" \
    -o /tmp/ffmpeg.zip
  unzip -o /tmp/ffmpeg.zip ffmpeg -d /tmp/ffmpeg-release
  mv /tmp/ffmpeg-release/ffmpeg "$LLAMA_DEST/ffmpeg"
  chmod +x "$LLAMA_DEST/ffmpeg"
  echo "   Bundled: $("$LLAMA_DEST/ffmpeg" -version 2>&1 | head -1)"
fi

# ── 4. llama.cpp (llama-server + llama-mtmd-cli) ─────────────────────────────
if [ "${SKIP_LLAMA_BUILD:-0}" = "1" ]; then
  echo "→ SKIP_LLAMA_BUILD=1 — skipping llama.cpp build."
elif [ -f "$LLAMA_DEST/llama-server" ] && [ -f "$LLAMA_DEST/llama-mtmd-cli" ]; then
  echo "→ llama-server and llama-mtmd-cli already bundled — skipping build."
  echo "   Delete them and re-run to force a rebuild."
else
  echo "→ Cloning and building llama.cpp with Metal…"
  echo "   (This takes 5-15 minutes depending on your Mac.)"
  rm -rf /tmp/llama-src
  git clone --depth 1 https://github.com/ggml-org/llama.cpp /tmp/llama-src
  cd /tmp/llama-src

  cmake -B build \
    -DCMAKE_BUILD_TYPE=Release \
    -DGGML_METAL=ON \
    -DGGML_METAL_EMBED_LIBRARY=ON \
    -DLLAMA_CURL=OFF \
    -DLLAMA_BUILD_TESTS=OFF \
    -DLLAMA_BUILD_EXAMPLES=ON \
    -DLLAMA_BUILD_SERVER=ON

  cmake --build build --config Release -j \
    --target llama-server \
    --target llama-mtmd-cli

  cp build/bin/llama-server   "$LLAMA_DEST/"
  cp build/bin/llama-mtmd-cli "$LLAMA_DEST/"

  find build -type f \( -name '*.dylib' -o -name 'default.metallib' \) \
    -exec cp {} "$LLAMA_DEST/" \;

  chmod +x "$LLAMA_DEST/llama-server" "$LLAMA_DEST/llama-mtmd-cli"

  # Fix dylib load paths to @loader_path so the .app bundle is relocatable.
  cd "$LLAMA_DEST"
  for f in llama-server llama-mtmd-cli *.dylib; do
    [ -f "$f" ] || continue
    otool -L "$f" | awk 'NR>1 {print $1}' | while read -r dep; do
      base=$(basename "$dep")
      if [ -f "$base" ] && [ "$dep" != "@loader_path/$base" ]; then
        install_name_tool -change "$dep" "@loader_path/$base" "$f" \
          2>/dev/null || true
      fi
    done
    if [[ "$f" == *.dylib ]]; then
      install_name_tool -id "@loader_path/$f" "$f" 2>/dev/null || true
    fi
  done

  echo "   Build complete."
fi

# ── 5. Playwright resources ───────────────────────────────────────────────────
echo "→ Syncing Playwright resources…"
cd "$REPO_ROOT"
node scripts/sync-playwright-resource.mjs

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "=== Resources ready ==="
echo "resources/llama/  :"
ls -lh "$LLAMA_DEST/" | grep -v "^total" | awk '{print "  " $5 "  " $9}'
echo "resources/node/   :"
ls -lh "$NODE_DEST/"  | grep -v "^total" | awk '{print "  " $5 "  " $9}'
echo ""
echo "All done. You can now run: npm run tauri dev"
