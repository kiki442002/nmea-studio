#!/usr/bin/env bash
set -euo pipefail

# Build optimized release binary and package it as a native macOS GUI bundle ("NMEA Studio.app")
# Launching "NMEA Studio.app" from Finder or Spotlight opens the GUI directly with NO Terminal window.

echo "Building NMEA Studio (Release)..."
cargo build --release

APP_DIR="dist/NMEA Studio.app"
rm -rf "${APP_DIR}"
mkdir -p "${APP_DIR}/Contents/MacOS"
mkdir -p "${APP_DIR}/Contents/Resources"

cp "target/release/nmea-studio" "${APP_DIR}/Contents/MacOS/nmea-studio"
chmod +x "${APP_DIR}/Contents/MacOS/nmea-studio"

cat > "${APP_DIR}/Contents/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>NMEA Studio</string>
    <key>CFBundleDisplayName</key>
    <string>NMEA Studio</string>
    <key>CFBundleIdentifier</key>
    <string>com.nmeastudio.app</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>nmea-studio</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
EOF

echo "Created macOS application bundle at: ${APP_DIR}"
echo "You can now double-click '${APP_DIR}' in Finder or copy it to /Applications."
