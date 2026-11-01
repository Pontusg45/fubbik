#!/usr/bin/env bash
# Builds fubbik desktop in release mode, wraps it in a macOS app bundle at
# ~/Applications/fubbik.app (or $FUBBIK_APP_DIR/fubbik.app), and pins it to
# the Dock unless it is already there or --no-dock is given.
#
# Re-run it after changes: the bundle is replaced, the Dock entry is kept.
set -euo pipefail

app_root="$(cd "$(dirname "$0")/.." && pwd)"
dest_dir="${FUBBIK_APP_DIR:-$HOME/Applications}"
app="$dest_dir/fubbik.app"
pin_dock=1
[[ "${1:-}" == "--no-dock" ]] && pin_dock=0

if [[ "$(uname)" != "Darwin" ]]; then
  echo "install-app.sh builds a macOS app bundle; this is $(uname)." >&2
  exit 1
fi

# libghostty-vt 0.2.2 runs `zig` from PATH and needs exactly Zig 0.15.2
# (brew install zig@0.15, keg-only).
export PATH="/opt/homebrew/opt/zig@0.15/bin:$PATH"
echo "Building release binary…"
cargo build --release --manifest-path "$app_root/Cargo.toml"
# Cargo honors CARGO_TARGET_DIR (just exports one); fall back to the crate's own target/.
binary="${CARGO_TARGET_DIR:-$app_root/target}/release/fubbik-desktop"
[[ -x "$binary" ]] || binary="$app_root/target/release/fubbik-desktop"

version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$app_root/Cargo.toml" | head -1)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "Drawing the icon…"
swift "$app_root/scripts/make-icon.swift" "$work/icon.png"
iconset="$work/AppIcon.iconset"
mkdir -p "$iconset"
for px in 16 32 128 256 512; do
  sips -z "$px" "$px" "$work/icon.png" --out "$iconset/icon_${px}x${px}.png" >/dev/null
  sips -z $((px * 2)) $((px * 2)) "$work/icon.png" --out "$iconset/icon_${px}x${px}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$work/AppIcon.icns"

echo "Assembling ${app}…"
mkdir -p "$dest_dir"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/fubbik-desktop"
cp "$work/AppIcon.icns" "$app/Contents/Resources/AppIcon.icns"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>fubbik</string>
  <key>CFBundleDisplayName</key><string>fubbik</string>
  <key>CFBundleIdentifier</key><string>dev.fubbik.desktop</string>
  <key>CFBundleExecutable</key><string>fubbik-desktop</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSAppleEventsUsageDescription</key><string>fubbik opens Terminal to start and resume AI sessions.</string>
</dict>
</plist>
PLIST
# Ad-hoc sign the bundle so macOS treats it as one app (and keeps its permissions).
codesign --force --deep --sign - "$app" >/dev/null
touch "$app"

if (( pin_dock )); then
  if defaults read com.apple.dock persistent-apps 2>/dev/null | grep -q "fubbik.app"; then
    echo "Already in the Dock."
  else
    echo "Adding to the Dock (the Dock restarts once)…"
    defaults write com.apple.dock persistent-apps -array-add \
      "<dict><key>tile-data</key><dict><key>file-data</key><dict><key>_CFURLString</key><string>file://${app}/</string><key>_CFURLStringType</key><integer>15</integer></dict></dict></dict>"
    killall Dock
  fi
fi
echo "Installed $app"
