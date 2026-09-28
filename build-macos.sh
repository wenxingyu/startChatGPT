#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"

if [[ "$(uname -s)" != Darwin ]]; then
    echo 'build-macos.sh requires macOS and Xcode Command Line Tools.' >&2
    exit 1
fi

target="${1:-aarch64-apple-darwin}"
case "$target" in
    aarch64-apple-darwin) swift_target=arm64-apple-macos11.0 ;;
    x86_64-apple-darwin) swift_target=x86_64-apple-macos11.0 ;;
    *) echo "Unsupported target: $target" >&2; exit 1 ;;
esac

rustup target add "$target"
export MACOSX_DEPLOYMENT_TARGET=11.0
cargo build --release --locked --target "$target"
output="target/$target/release"
swiftc -swift-version 5 -O -target "$swift_target" -framework AppKit \
    -module-cache-path "$output/swift-module-cache" \
    native/macos/main.swift -o "$output/startChatGPT-ui"

# Tests must run on the matching host architecture; builds can cross-compile.
host="$(rustc -vV | sed -n 's/^host: //p')"
if [[ "$host" == "$target" ]]; then
    cargo test --release --locked --target "$target"
    "$output/startChatGPT-ui" self-test
fi

bundle="$output/startChatGPT.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp "$output/startChatGPT" "$bundle/Contents/MacOS/startChatGPT"
cp "$output/startChatGPT-ui" "$bundle/Contents/Resources/startChatGPT-ui"
cp assets/chatgpt.icns "$bundle/Contents/Resources/chatgpt.icns"
version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
cat > "$bundle/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>startChatGPT</string>
  <key>CFBundleDisplayName</key><string>startChatGPT</string>
  <key>CFBundleIdentifier</key><string>io.github.wenxingyu.startChatGPT</string>
  <key>CFBundleExecutable</key><string>startChatGPT</string>
  <key>CFBundleIconFile</key><string>chatgpt.icns</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST

# An ad hoc signature permits local Apple Silicon execution. Public distribution
# needs a Developer ID signature and notarization, configured separately.
codesign --force --sign - "$bundle/Contents/Resources/startChatGPT-ui"
codesign --force --sign - "$bundle"
codesign --verify --strict "$bundle"
archive="$output/startChatGPT-macos-${target%%-apple-*}.zip"
ditto -c -k --sequesterRsrc --keepParent "$bundle" "$archive"
echo "Built $archive"
