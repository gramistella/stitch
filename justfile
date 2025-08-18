# Justfile

# Justfile
dmg:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v cargo-bundle >/dev/null 2>&1 || cargo install cargo-bundle
    cargo bundle --release --format osx
    APP_PATH="$(ls -d target/release/bundle/*/*.app | head -n1)"
    APP_NAME="$(basename "$APP_PATH" .app)"
    VERSION="$(sed -nE 's/^[[:space:]]*version[[:space:]]*=[[:space:]]*"([^"]+)".*$/\1/p' Cargo.toml | head -n1)"
    mkdir -p dist
    hdiutil create -volname "$APP_NAME" -srcfolder "$APP_PATH" -ov -format UDZO "dist/${APP_NAME}-${VERSION}.dmg"
    echo "✅ dist/${APP_NAME}-${VERSION}.dmg"

