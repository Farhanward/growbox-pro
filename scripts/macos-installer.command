#!/bin/bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP=$(find "$SCRIPT_DIR" -name "*.app" -maxdepth 1 | head -1)
APP_NAME=$(basename "$APP")

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Installing $APP_NAME"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

if [ -d "/Applications/$APP_NAME" ]; then
  echo "Removing previous version..."
  rm -rf "/Applications/$APP_NAME"
fi

echo "Copying to /Applications ..."
cp -R "$APP" /Applications/

echo "Removing macOS security restriction..."
xattr -cr "/Applications/$APP_NAME"

echo ""
echo "Installed! Launching $APP_NAME ..."
echo ""
open "/Applications/$APP_NAME"
