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

xattr -cr "${APP_DIR}"
codesign --force --deep --sign - "${APP_DIR}"
codesign --verify --deep --strict "${APP_DIR}"

# Stage DMG folder with "NMEA Studio.app" and "/Applications" symlink for drag-and-drop install
DMG_STAGE="dist/dmg_stage"
DMG_PATH="dist/NMEA-Studio-macOS.dmg"
rm -rf "${DMG_STAGE}" "${DMG_PATH}"
mkdir -p "${DMG_STAGE}"
cp -R "${APP_DIR}" "${DMG_STAGE}/NMEA Studio.app"
ln -s /Applications "${DMG_STAGE}/Applications"

hdiutil create -volname "NMEA Studio" -srcfolder "${DMG_STAGE}" -ov -format UDZO "${DMG_PATH}"
codesign --force --sign - "${DMG_PATH}"
rm -rf "${DMG_STAGE}"

echo "Created and signed macOS application bundle at: ${APP_DIR}"
echo "Created drag-and-drop DMG installer at: ${DMG_PATH}"
