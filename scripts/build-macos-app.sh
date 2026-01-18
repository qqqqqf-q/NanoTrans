#!/usr/bin/env bash
set -euo pipefail

APP_NAME="NanoTrans"
VERSION=$(grep -E '^\s*version\s*=' Cargo.toml | head -n1 | sed -E 's/.*"([^"]+)".*/\1/')
if [ -z "$VERSION" ]; then
  echo "Cargo.toml 中未找到 version 字段" >&2
  exit 1
fi

cargo build --release

APP_DIR="dist/${APP_NAME}.app"
BIN_PATH="target/release/nanotrans"
ICONSET_DIR="target/app-icons/${APP_NAME}.iconset"
ICON_FILE="${APP_NAME}.icns"
ICON_PATH="target/app-icons/${ICON_FILE}"
mkdir -p "${APP_DIR}/Contents/MacOS" "${APP_DIR}/Contents/Resources"

if [ -d "${ICONSET_DIR}" ]; then
  iconutil -c icns "${ICONSET_DIR}" -o "${ICON_PATH}"
fi

if [ ! -f "${ICON_PATH}" ]; then
  echo "未生成 ${ICON_PATH}，请确认 iconutil 是否可用" >&2
  exit 1
fi

cp "${BIN_PATH}" "${APP_DIR}/Contents/MacOS/nanotrans"
chmod +x "${APP_DIR}/Contents/MacOS/nanotrans"
cp "${ICON_PATH}" "${APP_DIR}/Contents/Resources/${ICON_FILE}"

cat > "${APP_DIR}/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleDisplayName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleIdentifier</key>
  <string>com.nanotrans.app</string>
  <key>CFBundleExecutable</key>
  <string>nanotrans</string>
  <key>CFBundleIconFile</key>
  <string>${ICON_FILE}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.productivity</string>
  <key>LSUIElement</key>
  <true/>
</dict>
</plist>
EOF

ditto -c -k --sequesterRsrc --keepParent "${APP_DIR}" "${APP_NAME}-macOS.app.zip"
echo "输出完成：${APP_NAME}-macOS.app.zip"
