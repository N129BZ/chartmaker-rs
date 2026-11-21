#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "=== 1. Building Release Binary ==="
cargo build --release

PACKAGE_NAME="chartmaker-linux-x86_64"
DIST_DIR="$ROOT_DIR/target/dist"
STAGING_DIR="$DIST_DIR/$PACKAGE_NAME"

echo "=== 2. Assembling Staging Package ==="
rm -rf "$DIST_DIR"
mkdir -p "$STAGING_DIR"

# Copy binary
cp "$ROOT_DIR/target/release/chartmaker" "$STAGING_DIR/"

# Copy runtime assets and config
cp "$ROOT_DIR/config.yml" "$STAGING_DIR/"
cp "$ROOT_DIR/chartdates.json" "$STAGING_DIR/"
cp -r "$ROOT_DIR/dist" "$STAGING_DIR/"
mkdir -p "$STAGING_DIR/clipshapes"
cp "$ROOT_DIR/clipshapes/clipshapes.geojson" "$STAGING_DIR/clipshapes/"

# Create runtime directories
mkdir -p "$STAGING_DIR/chartcache" "$STAGING_DIR/mbtiles" "$STAGING_DIR/workarea"

# Add launcher script
cat << 'EOF' > "$STAGING_DIR/start.sh"
#!/usr/bin/env bash
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$DIR"

# Check GDAL prerequisite
if ! command -v gdalwarp &> /dev/null; then
    echo "[WARNING] GDAL command line tools (gdalwarp, gdal_translate, etc.) were not found in PATH."
    echo "Please install them via: sudo apt install gdal-bin curl unzip zip"
    exit 1
fi

WEBSERVER=true ./chartmaker
EOF
chmod +x "$STAGING_DIR/start.sh" "$STAGING_DIR/chartmaker"

echo "=== 3. Creating Tarball Archive ==="
cd "$DIST_DIR"
tar -czf "${PACKAGE_NAME}.tar.gz" "$PACKAGE_NAME"

echo "=== Done! Package created at target/dist/${PACKAGE_NAME}.tar.gz ==="
