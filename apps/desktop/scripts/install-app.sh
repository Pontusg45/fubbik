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
# The embedded backend links fubbik-db; build its sqlx queries from .sqlx/.
SQLX_OFFLINE=true cargo build --release --manifest-path "$app_root/Cargo.toml"
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
# The embedded backend's PostgreSQL: a trimmed copy of Postgres.app 18 (it
# ships pgvector and is relocatable: its binaries load libraries through
# @loader_path). Only the server tools, the libraries they load, and
# Postgres's own lib/ and share/ trees; PostGIS and GDAL stay behind.
# Override the source with FUBBIK_POSTGRES_SRC (a Versions/<n> folder).
pg_src="${FUBBIK_POSTGRES_SRC:-/Applications/Postgres.app/Contents/Versions/18}"
if [[ -x "$pg_src/bin/postgres" && -f "$pg_src/share/postgresql/extension/vector.control" ]]; then
  echo "Bundling PostgreSQL from ${pg_src}…"
  pg_dest="$app/Contents/Resources/postgres"
  mkdir -p "$pg_dest/bin" "$pg_dest/lib"
  tools=(postgres initdb pg_ctl pg_config pg_dump pg_restore)
  for tool in "${tools[@]}"; do cp -p "$pg_src/bin/$tool" "$pg_dest/bin/"; done
  # Every @loader_path/../lib dylib the tools need, followed transitively.
  queue=()
  for tool in "${tools[@]}"; do queue+=("$pg_src/bin/$tool"); done
  seen=" "
  while ((${#queue[@]})); do
    file="${queue[0]}"; queue=("${queue[@]:1}")
    while read -r lib; do
      name="${lib##*/}"
      [[ "$seen" == *" $name "* ]] && continue
      seen+="$name "
      cp -p "$pg_src/lib/$name" "$pg_dest/lib/"
      queue+=("$pg_src/lib/$name")
    done < <(otool -L "$file" | awk '/@loader_path\/(\.\.\/lib\/)?[^ ]+\.dylib/ {print $1}')
  done
  # Postgres's own modules, without PostGIS/pgRouting, the language
  # handlers fubbik never loads, and Postgres.app's permission dialog.
  rsync -a \
    --exclude 'pgxs' --exclude 'postgis*' --exclude 'address_standardizer*' --exclude 'libpgrouting*' \
    --exclude 'pljs*' --exclude '*plpython3*' --exclude 'plugin_debugger*' --exclude 'auth_permission_dialog*' \
    "$pg_src/lib/postgresql" "$pg_dest/lib/"
  # share/: everything initdb needs, but only the extensions fubbik uses.
  mkdir -p "$pg_dest/share/postgresql/extension"
  rsync -a --exclude 'extension' --exclude 'contrib' "$pg_src/share/postgresql/" "$pg_dest/share/postgresql/"
  for ext in vector pg_trgm plpgsql; do
    cp -p "$pg_src/share/postgresql/extension/${ext}".control "$pg_src/share/postgresql/extension/${ext}"--*.sql \
      "$pg_dest/share/postgresql/extension/"
  done
  # Apache AGE (graph projection), built once by build-age.sh and cached.
  # FUBBIK_SKIP_AGE=1 leaves it out (no clone or build).
  if [[ "${FUBBIK_SKIP_AGE:-}" == 1 ]]; then
    echo "  without Apache AGE (FUBBIK_SKIP_AGE=1)"
  elif age_stage="$(FUBBIK_POSTGRES_SRC="$pg_src" "$app_root/scripts/build-age.sh")"; then
    cp -p "${age_stage}/lib/age.dylib" "$pg_dest/lib/postgresql/"
    cp -p "${age_stage}"/extension/age* "$pg_dest/share/postgresql/extension/"
    outside="$(otool -L "$pg_dest/lib/postgresql/age.dylib" | tail -n +2 | awk '{print $1}' | grep -vE '^(/usr/lib/|/System/|@loader_path|@rpath)' || true)"
    [[ -z "$outside" ]] || echo "  warning: age.dylib links outside the bundle: ${outside}"
    echo "  with Apache AGE"
  else
    echo "  without Apache AGE (build-age.sh failed; graph projection stays off)"
  fi
  "$pg_dest/bin/postgres" --version >/dev/null
  echo "  $(du -sh "$pg_dest" | cut -f1) — $("$pg_dest/bin/postgres" --version)"
else
  echo "No Postgres.app 18 with pgvector at $pg_src: the app will look for one when it starts."
fi

# The web UI, served by the embedded API at http://127.0.0.1:3000.
web_dist="$app_root/../web/dist"
if [[ -f "$web_dist/index.html" ]]; then
  cp -Rp "$web_dist" "$app/Contents/Resources/web"
  echo "Bundled the web UI from apps/web/dist."
else
  echo "No apps/web/dist build (run: pnpm build): the embedded API serves only /api."
fi

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>fubbik</string>
  <key>CFBundleDisplayName</key><string>fubbik</string>
  <key>CFBundleIdentifier</key><string>dev.fubbik.desktop</string>
  <key>CFBundleURLTypes</key>
  <array>
    <dict>
      <key>CFBundleURLName</key><string>dev.fubbik.desktop</string>
      <key>CFBundleURLSchemes</key><array><string>fubbik</string></array>
    </dict>
  </array>
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
