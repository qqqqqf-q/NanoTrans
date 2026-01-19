# NanoTrans

NanoTrans is an ultra-lightweight, cross-platform input translation assistant for Windows and macOS. It integrates at the system level and locks onto the caret position to provide instant translation and in-place refilling.

# Project is still under development and the current version is not recommended for trial use
Current version: 0.1.0-alpha.2

## Download

- Windows (exe): https://github.com/qqqqqf-q/NanoTrans/releases/download/latest/nanotrans.exe
- macOS (app): https://github.com/qqqqqf-q/NanoTrans/releases/download/latest/NanoTrans-macOS.app.zip

## Platform support

- **Windows**: Full feature support with accurate caret position detection
- **macOS**: Core features supported (requires accessibility permissions)

## How it works

1. Type text in any input field.
2. Press Ctrl+A.
3. Press Alt+Q (translation hotkey).
4. Press Ctrl+V.
5. You get the translated text in place.

## Core features

1. Caret tracking:
   - Windows: Uses the GetGUIThreadInfo API to lock the caret position in the input field
   - macOS: Uses mouse position as a fallback
   - Ensures the translation popup appears near the input focus
2. In-place translation and refilling: a global hotkey triggers Select -> Copy -> Translate -> Show -> Auto-paste to replace the input quickly.
3. Ultra lightweight: no browser engine, native rendering, instant startup, minimal memory footprint, and disk size under 3 MB.
4. System tray resident: runs silently in the background and provides configuration via tray menu.
5. Cross-platform support: a single codebase for Windows and macOS.

## Technical approach

1. Core language: Rust.
2. UI framework: Slint, using a declarative syntax compiled to native machine code with efficient GPU rendering.
3. System interfaces:
   - Windows: windows-rs, direct Win32 API calls
   - macOS: cocoa, core-graphics, core-foundation
4. Clipboard: arboard for cross-platform clipboard access.
5. Networking: reqwest (with rustls-tls) for lightweight async API requests.
6. Cross-platform architecture: modular design with platform-specific code isolated in dedicated modules.

## Usage

1. Configure: right-click the system tray icon and enter your translation service API key.
2. Trigger: select the text in any editor or input field.
3. Convert: press the preset hotkey (default Ctrl+Shift+T / Cmd+Shift+T on macOS).
4. Replace: the translation appears at the caret; confirm by clicking or pressing Enter.

### macOS notes

On first run, grant accessibility permissions:
1. Open "System Preferences" > "Security & Privacy" > "Accessibility"
2. Add NanoTrans to the allowed list
3. Restart the application

If the downloaded .app cannot be opened, run:
`xattr -dr com.apple.quarantine NanoTrans.app`

## Build and development

To build from source and achieve minimal size, install the Rust toolchain first.

```bash
# Get the source
git clone https://github.com/yourname/NanoTrans.git
cd NanoTrans

# Release build
# This build uses LTO (Link Time Optimization) and Strip
cargo build --release

# Windows: optional WiX Toolset for MSI packaging
# macOS: the binary is in target/release/nanotrans
```

macOS app packaging:

```bash
./scripts/build-macos-app.sh
```

## Size optimization

This project uses the following Cargo.toml settings to reduce binary size:

* Set optimization level to z (opt-level = "z") for smaller binaries.
* Enable link-time optimization (lto = true).
* Strip all debug symbols (strip = true).
* Reduce the number of codegen units to improve optimization.

## License

MIT
