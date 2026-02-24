#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MACOS_DIR="$ROOT_DIR/macos"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"
VERSION="${VERSION:-${GITHUB_REF_NAME:-$(git -C "$ROOT_DIR" describe --tags --always --dirty)}}"

cd "$MACOS_DIR"
swift build -c release --product WhisperAssistant
swift build -c release --product wa

rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

APP_DIR="$DIST_DIR/WhisperAssistant.app"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources" "$APP_DIR/Contents/Frameworks"

cp ".build/release/WhisperAssistant" "$APP_DIR/Contents/MacOS/WhisperAssistant"
chmod +x "$APP_DIR/Contents/MacOS/WhisperAssistant"

SPARKLE_SOURCE=""
if [[ -d ".build/release/Sparkle.framework" ]]; then
  SPARKLE_SOURCE=".build/release/Sparkle.framework"
elif [[ -d ".build/arm64-apple-macosx/release/Sparkle.framework" ]]; then
  SPARKLE_SOURCE=".build/arm64-apple-macosx/release/Sparkle.framework"
fi

if [[ -n "$SPARKLE_SOURCE" ]]; then
  cp -R "$SPARKLE_SOURCE" "$APP_DIR/Contents/Frameworks/Sparkle.framework"
fi

# Make sure bundled frameworks are discoverable by @rpath.
if ! otool -l "$APP_DIR/Contents/MacOS/WhisperAssistant" | grep -q "@executable_path/../Frameworks"; then
  install_name_tool -add_rpath "@executable_path/../Frameworks" "$APP_DIR/Contents/MacOS/WhisperAssistant"
fi

cat > "$APP_DIR/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>WhisperAssistant</string>
  <key>CFBundleIdentifier</key>
  <string>com.whisperassistant.app</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>WhisperAssistant</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key>
  <string>14.0</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSMicrophoneUsageDescription</key>
  <string>Whisper Assistant needs microphone access to record your voice for transcription.</string>
</dict>
</plist>
EOF

APP_ZIP="$DIST_DIR/WhisperAssistant.app.zip"
DMG_PATH="$DIST_DIR/WhisperAssistant.dmg"
CLI_ZIP="$DIST_DIR/wa-macos.zip"
ENTITLEMENTS_PATH="$DIST_DIR/WhisperAssistant.entitlements"

cat > "$ENTITLEMENTS_PATH" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>com.apple.security.device.audio-input</key>
  <true/>
</dict>
</plist>
EOF

# Optional signing when identity is available in runner keychain.
if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  if [[ -d "$APP_DIR/Contents/Frameworks/Sparkle.framework" ]]; then
    # Sparkle ships nested helper executables (Updater/Autoupdate/XPC services) that
    # must be re-signed with our Developer ID identity for notarization to pass.
    codesign --force --timestamp --options runtime --deep --sign "${APPLE_SIGNING_IDENTITY}" "$APP_DIR/Contents/Frameworks/Sparkle.framework"
  fi
  codesign --force --timestamp --options runtime --sign "${APPLE_SIGNING_IDENTITY}" "$APP_DIR/Contents/MacOS/WhisperAssistant"
  codesign --force --timestamp --options runtime --entitlements "$ENTITLEMENTS_PATH" --sign "${APPLE_SIGNING_IDENTITY}" "$APP_DIR"
else
  # Ensure unsigned builds still have a structurally valid bundle signature.
  # Without this, Gatekeeper can report "is damaged" for linker-signed binaries.
  if [[ -d "$APP_DIR/Contents/Frameworks/Sparkle.framework" ]]; then
    codesign --force --sign - "$APP_DIR/Contents/Frameworks/Sparkle.framework"
  fi
  codesign --force --sign - "$APP_DIR/Contents/MacOS/WhisperAssistant"
  codesign --force --sign - "$APP_DIR"
fi

ditto -c -k --sequesterRsrc --keepParent "$APP_DIR" "$APP_ZIP"
hdiutil create -volname "WhisperAssistant" -srcfolder "$APP_DIR" -ov -format UDZO "$DMG_PATH" >/dev/null
ditto -c -k --sequesterRsrc ".build/release/wa" "$CLI_ZIP"

if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]] && [[ -n "${APPLE_ID:-}" ]] && [[ -n "${APPLE_TEAM_ID:-}" ]] && [[ -n "${APPLE_APP_PASSWORD:-}" ]]; then
  NOTARY_RESULT_JSON="$DIST_DIR/notary-result.json"
  xcrun notarytool submit "$DMG_PATH" \
    --apple-id "${APPLE_ID}" \
    --team-id "${APPLE_TEAM_ID}" \
    --password "${APPLE_APP_PASSWORD}" \
    --wait \
    --output-format json > "$NOTARY_RESULT_JSON"

  NOTARY_STATUS="$(plutil -extract status raw -o - "$NOTARY_RESULT_JSON" 2>/dev/null || true)"
  if [[ "$NOTARY_STATUS" != "Accepted" ]]; then
    NOTARY_ID="$(plutil -extract id raw -o - "$NOTARY_RESULT_JSON" 2>/dev/null || true)"
    if [[ -n "$NOTARY_ID" ]]; then
      xcrun notarytool log "$NOTARY_ID" \
        --apple-id "${APPLE_ID}" \
        --team-id "${APPLE_TEAM_ID}" \
        --password "${APPLE_APP_PASSWORD}" || true
    fi
    echo "Notarization failed with status: ${NOTARY_STATUS:-unknown}" >&2
    exit 1
  fi

  xcrun stapler staple "$DMG_PATH"
fi

(
  cd "$DIST_DIR"
  shasum -a 256 "$(basename "$APP_ZIP")" "$(basename "$DMG_PATH")" "$(basename "$CLI_ZIP")" > SHA256SUMS.txt
)

echo "Release artifacts ready in $DIST_DIR"
