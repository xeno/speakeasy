#!/usr/bin/env bash
# Build a SpeakEasy.app for distribution.
# Does not copy Application Support, .env, or any Gemini key.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$ROOT/dist"
VENDOR="$DIST/vendor"
APP="$DIST/SpeakEasy.app"
MACOS="$APP/Contents/MacOS"
RESOURCES="$APP/Contents/Resources"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)"

echo "==> Refusing to bundle secrets"
for leak in \
  "$ROOT/.env" \
  "$ROOT/.env.local" \
  "$ROOT/config.yaml" \
  "$ROOT/src/.env"
do
  if [[ -e "$leak" ]]; then
    echo "    warning: $leak exists in the repo and will not be copied"
  fi
done

echo "==> Release build"
cargo build --release --manifest-path "$ROOT/Cargo.toml"

echo "==> ffmpeg / ffprobe for the bundle"
mkdir -p "$VENDOR"
fetch_evermeet() {
  local name="$1"
  local dest="$VENDOR/$name"
  if [[ -x "$dest" ]]; then
    return 0
  fi
  local zip="$VENDOR/$name.zip"
  curl -fsSL "https://evermeet.cx/ffmpeg/getrelease/$name/zip" -o "$zip" || return 1
  unzip -o -q "$zip" -d "$VENDOR" || return 1
  rm -f "$zip"
  chmod +x "$dest"
}

if fetch_evermeet ffmpeg && fetch_evermeet ffprobe; then
  echo "    using static ffmpeg from evermeet.cx"
else
  echo "    evermeet.cx download failed; using ffmpeg from PATH (may have Homebrew dylibs)"
  command -v ffmpeg >/dev/null || { echo "ffmpeg is required"; exit 1; }
  command -v ffprobe >/dev/null || { echo "ffprobe is required"; exit 1; }
  cp "$(command -v ffmpeg)" "$VENDOR/ffmpeg"
  cp "$(command -v ffprobe)" "$VENDOR/ffprobe"
  chmod +x "$VENDOR/ffmpeg" "$VENDOR/ffprobe"
fi

echo "==> Assemble SpeakEasy.app"
rm -rf "$APP"
mkdir -p "$MACOS" "$RESOURCES"
cp "$ROOT/target/release/speakeasy" "$MACOS/speakeasy"
cp "$VENDOR/ffmpeg" "$MACOS/ffmpeg"
cp "$VENDOR/ffprobe" "$MACOS/ffprobe"
chmod +x "$MACOS/speakeasy" "$MACOS/ffmpeg" "$MACOS/ffprobe"
sed "s/0.1.0/${VERSION}/g" "$ROOT/packaging/Info.plist" > "$APP/Contents/Info.plist"

echo "==> Secret scan"
if find "$APP" \( -name '.env*' -o -name 'config.yaml' -o -name '*.pem' \) | grep -q .; then
  echo "Refusing to ship: secret-like files found in the bundle"
  find "$APP" \( -name '.env*' -o -name 'config.yaml' -o -name '*.pem' \)
  exit 1
fi
if grep -R -l -E 'AIza[0-9A-Za-z_-]{20,}|gemini_api_key:' "$APP" >/dev/null 2>&1; then
  echo "Refusing to ship: a Gemini key string was found in the bundle"
  exit 1
fi

if [[ -n "${CODESIGN_IDENTITY:-}" ]]; then
  echo "==> codesign ($CODESIGN_IDENTITY)"
  codesign --force --options runtime \
    --entitlements "$ROOT/packaging/entitlements.plist" \
    --sign "$CODESIGN_IDENTITY" \
    "$MACOS/ffmpeg" "$MACOS/ffprobe" "$MACOS/speakeasy" "$APP"
else
  echo "==> skipping codesign (set CODESIGN_IDENTITY to sign)"
fi

ZIP="$DIST/SpeakEasy-$VERSION-macos.zip"
echo "==> $ZIP"
rm -f "$ZIP"
ditto -c -k --keepParent "$APP" "$ZIP"

echo "Done."
echo "  $APP"
echo "  $ZIP"
echo "Gemini keys stay in ~/Library/Application Support/SpeakEasy/config.yaml — not in this bundle."
