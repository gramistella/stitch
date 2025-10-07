# Justfile
# Use bash everywhere (including on Windows runners via "shell: bash" in Actions)
set shell := ["bash", "-euo", "pipefail", "-c"]
set windows-shell := ["bash", "-euo", "pipefail", "-c"]

test:
	cargo test --workspace --all-targets --all-features

lint:
  cargo clippy --workspace --all-targets --all-features -- \
    -W clippy::all -W clippy::cargo -W clippy::pedantic -W clippy::nursery -A clippy::multiple-crate-versions -D warnings

fmt:
	cargo fmt --all

# Build the UI app in release mode.
# Optional: pass TARGET=triple to cross-compile (e.g., x86_64-unknown-linux-musl)
bin-release:
	#!/usr/bin/env bash
	if [[ -n "${TARGET:-}" ]]; then
	  rustup target add "$TARGET" || true
	  cargo build --release --features ui --target "$TARGET"
	else
	  cargo build --release --features ui,tokens
	fi

# macOS: build .app and .dmg
dmg:
	#!/usr/bin/env bash
	VERSION="$(python -c 'import re,pathlib;print(re.search(r"^version\s*=\s*\"([^\"]+)\"", pathlib.Path("Cargo.toml").read_text(), re.M).group(1))')"
	command -v cargo-bundle >/dev/null 2>&1 || cargo install cargo-bundle
	cargo bundle --release --format osx
	APP_PATH="$(ls -d target/release/bundle/*/*.app | head -n1)"
	APP_NAME="$(basename "$APP_PATH" .app)"
	mkdir -p dist
	hdiutil create -volname "$APP_NAME" -srcfolder "$APP_PATH" -ov -format UDZO "dist/${APP_NAME}-${VERSION}.dmg"
	echo "✅ dist/${APP_NAME}-${VERSION}.dmg"

# Windows: build and zip stitch.exe (portable, no installer)
exe: bin-release
	#!/usr/bin/env bash
	VERSION="$(python -c 'import re,pathlib;print(re.search(r"^version\s*=\s*\"([^\"]+)\"", pathlib.Path("Cargo.toml").read_text(), re.M).group(1))')"
	mkdir -p dist
	if [[ -n "${TARGET:-}" ]]; then
	  BIN="target/$TARGET/release/stitch.exe"
	else
	  BIN="target/release/stitch.exe"
	fi
	OUT="dist/stitch-${VERSION}-windows-x86_64.zip"
	if [[ ! -f "$BIN" ]]; then
	  echo "❌ $BIN not found (are you on a Windows runner?)" >&2
	  exit 1
	fi
	if command -v zip >/dev/null 2>&1; then
	  zip -9 "$OUT" "$BIN"
	elif command -v 7z >/dev/null 2>&1; then
	  7z a -tzip "$OUT" "$BIN" >/dev/null
	else
	  powershell -NoProfile -Command "Compress-Archive -Path '$BIN' -DestinationPath '$OUT' -Force"
	fi
	echo "✅ $OUT"

# Linux: build and tar-gzip the binary (portable tarball)
tgz: bin-release
	#!/usr/bin/env bash
	VERSION="$(python -c 'import re,pathlib;print(re.search(r"^version\s*=\s*\"([^\"]+)\"", pathlib.Path("Cargo.toml").read_text(), re.M).group(1))')"
	mkdir -p dist
	if [[ -n "${TARGET:-}" ]]; then
	  BIN="target/$TARGET/release/stitch"
	  ARCH="${TARGET%%-*}"  # e.g., x86_64 from x86_64-unknown-linux-musl
	  FLAV="$(echo "$TARGET" | grep -q musl && echo musl || echo gnu)"
	  OUT="dist/stitch-${VERSION}-linux-${ARCH}-${FLAV}.tar.gz"
	else
	  BIN="target/release/stitch"
	  ARCH="$(uname -m)"
	  OUT="dist/stitch-${VERSION}-linux-${ARCH}.tar.gz"
	fi
	if [[ -f "$BIN" ]]; then
	  tar -C "$(dirname "$BIN")" -czf "$OUT" "$(basename "$BIN")"
	  echo "✅ $OUT"
	else
	  echo "❌ $BIN not found (are you building on Linux?)" >&2
	  exit 1
	fi
