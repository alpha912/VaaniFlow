# Changelog

All notable changes to VaaniFlow will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-02-24

### Added

- System tray right-click menu with Settings and Quit options
- Custom floating bubble context menu (right-click) with Settings and Quit
- Tabler Icons throughout the UI (replacing emoji icons)

### Improved

- Redesigned settings window with card-based layout and proper typography
- Side-by-side language and hotkey/microphone fields for compact layout
- Hotkey display normalized to title case (e.g. Ctrl+Shift+Space)
- Target language "English" label simplified (removed "Indian accent")
- Window height auto-fits all settings without scrolling

### Fixed

- Quit button now works correctly (ExitRequested handler no longer blocks explicit exits)
- Native context menu replaced with custom HTML menu (fixes silent failure in transparent windows)

## [0.1.0] - 2026-02-24

### Added

- Floating bubble UI that follows text field focus across applications
- Speech-to-text using Sarvam Saaras v3 for 22+ Indian languages
- Auto-detect source language mode
- Live translation via Sarvam Mayura v1 to any supported Indian language
- Direct speech-to-English translation (single API call)
- Global hotkey (Ctrl+Shift+Space, customisable)
- Microphone picker in Settings
- API key validation in Settings
- System tray integration
- Automatic paste into focused text field via clipboard + Ctrl+V simulation
- Audio resampling to 16kHz for API compatibility
- Silent microphone detection with automatic redirect to Windows privacy settings
- Configuration persistence in OS app data directory

### Platform Support

- Windows 10/11 (primary, fully supported)
- macOS (scaffolded, needs contributor testing)

[0.2.0]: https://github.com/alpha912/VaaniFlow/releases/tag/v0.2.0
[0.1.0]: https://github.com/alpha912/VaaniFlow/releases/tag/v0.1.0
