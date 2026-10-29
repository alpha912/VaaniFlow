<p align="center">
  <img src="native/icons/icon.png" alt="VaaniFlow logo" width="120" />
</p>

<h1 align="center">VaaniFlow</h1>

<p align="center">
  <strong>Speak an Indian language. Get the words where you need them.</strong>
</p>

<p align="center">
  <a href="https://github.com/alpha912/VaaniFlow/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square" alt="License: MIT" /></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-native-DEA584?style=flat-square&logo=rust&logoColor=white" alt="Native Rust" /></a>
  <a href="https://github.com/emilk/egui"><img src="https://img.shields.io/badge/UI-egui-5B8DEF?style=flat-square" alt="egui" /></a>
  <a href="https://www.sarvam.ai/"><img src="https://img.shields.io/badge/API-Sarvam%20AI-E8671A?style=flat-square" alt="Sarvam AI" /></a>
  <a href="https://github.com/alpha912/VaaniFlow/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/alpha912/VaaniFlow/ci.yml?style=flat-square&label=CI" alt="CI" /></a>
</p>

VaaniFlow v0.4 is a native Rust desktop app with an egui interface. It captures microphone recordings up to two minutes, splits audio into bounded Sarvam requests, performs speech recognition and optional translation, and helps enter the resulting text into another app. Recordings can be discarded or retried after a failed request. Cross-application focus detection and input simulation are platform-dependent; see [Platform status](#platform-status) before relying on those integrations.

## What it does

- Captures microphone audio with a configurable input device and a two-minute recording limit; recordings are split into requests under 27 seconds each.
- Lets you discard a recording, retry a failed request, and refresh the microphone list.
- Uses Sarvam Saaras speech-to-text (v3 by default; v4 can be selected) and Mayura v1 for optional translation.
- Supports Sarvam's available languages and speech modes; exact availability depends on the selected model and Sarvam account.
- Provides configurable source/target languages, a global shortcut, translation style and terminology hints.
- Defaults to review-before-insert and keeps the Sarvam credential separate from ordinary preferences using the operating-system credential store when available. Transcribed text remains available if the optional translation step fails.
- Keeps a transcription/translation result or a saved retry available after a failed request; recordings awaiting retry remain in memory until retried, forgotten, discarded by starting a new recording, or the app exits.
- Sends requests directly from the app to Sarvam. Dictation audio and text are sent to that service when you use transcription or translation; VaaniFlow does not provide an intermediary service.

## Technology

| Area | Technology |
| :--- | :--- |
| Desktop UI | Rust, egui via eframe |
| Audio | cpal capture, mono conversion, WAV encoding with hound |
| Networking | reqwest with rustls, Tokio |
| Preferences and credentials | serde JSON preferences, OS keyring for the API key |
| Desktop integration | global-hotkey, clipboard and input simulation; platform-specific focus handling |
| Speech recognition | Sarvam Saaras v3 / v4 |
| Translation | Sarvam Mayura v1 |

## Build and run from source

### Prerequisites

- Rust stable, installed with [rustup](https://rustup.rs/).
- A [Sarvam AI API key](https://www.sarvam.ai/) to use transcription and translation.
- Platform build tools and native libraries:
  - **Windows:** Visual Studio Build Tools with the C++ build tools workload (MSVC).
  - **macOS:** Xcode Command Line Tools (`xcode-select --install`).
  - **Ubuntu/Debian:** the audio, window-system and X11 development packages below; use your distribution's equivalents elsewhere.

```bash
sudo apt-get update
sudo apt-get install --yes build-essential pkg-config \
  libasound2-dev libdbus-1-dev libudev-dev libwayland-dev \
  libx11-dev libxcursor-dev libxinerama-dev libxi-dev libxrandr-dev \
  libxkbcommon-dev libxkbcommon-x11-dev libxtst-dev
```

### Run and test

From the repository root:

```bash
# Clone the repository
git clone https://github.com/alpha912/VaaniFlow.git
cd VaaniFlow

# Run the native workspace app
cargo run -p vaaniflow
```

Enter your Sarvam API key in the app's settings; do not commit credentials or place them in a checked-in file. The Rust workspace and lockfile are at the repository root, and the native package is `vaaniflow` under `native/`.

To build and check the project locally:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build -p vaaniflow --release --locked
```

No JavaScript package installation or desktop-webview CLI is required; the UI and application logic are built in Rust.

## Platform status

The release workflow attempts native builds on Windows, macOS and Linux. A successful compile is not a guarantee that every cross-app interaction works in every desktop session:

- **Windows:** active text-field detection uses a best-effort heuristic for common native controls and applications. Focus restoration can be denied by Windows foreground policy.
- **macOS:** focus detection is hotkey-first rather than querying accessibility data. Input simulation and credential-store behavior may depend on the user's permissions and system configuration.
- **Linux:** focus detection is hotkey-first. Global hotkeys are X11-dependent; Wayland compositor policy and desktop portals can limit global shortcuts, clipboard access or simulated input. Install the distribution's audio and window-system libraries.

The automated release assets are unsigned portable binaries, not platform installers. They are not code-signed or notarized, and Linux binaries depend on compatible system libraries. Treat the multi-platform builds as early distribution artifacts, not a claim of complete platform parity.

## Data and settings

VaaniFlow stores preferences such as language, microphone, shortcut, speech mode and translation style in a per-user configuration file. The API key is stored separately in the operating-system keyring where supported. If secure credential storage cannot be accessed, the app may retain the key only for the current session and ask you to enter it again next time. Do not share logs or recordings containing sensitive information.

A transcription request sends the recorded audio to Sarvam's API. Translation sends the transcript to Sarvam as well. Review Sarvam's terms and privacy information before use.

## Release process

The version in `native/Cargo.toml` is the release version for the native app. The tag workflow runs formatting, Clippy and tests on the exact tagged source before building Windows, macOS (the hosted runner's architecture) and Linux archives. Before a release:

1. Update the native crate version and regenerate/check the workspace `Cargo.lock` as needed.
2. Run the formatting, Clippy, tests and release build commands above, and wait for CI to pass on the changes.
3. Create and push a version tag that exactly matches the crate version. For example, for `0.4.0`:

   ```bash
   git tag -a v0.4.0 -m "VaaniFlow v0.4.0"
   git push origin v0.4.0
   ```

4. The workflow creates a **draft** GitHub Release with unsigned portable binaries attached. Review the artifacts and notes, smoke-test on each supported platform/session, then publish the draft when ready.

Do not reuse a published version tag. If a build fails, fix the source and publish a new patch version/tag rather than moving the old tag.

## Project layout

```text
VaaniFlow/
├── Cargo.toml                 # Rust workspace
├── Cargo.lock                 # Workspace dependency lockfile
├── native/
│   ├── Cargo.toml             # vaaniflow 0.4 native package (egui/eframe)
│   ├── icons/                 # App icons
│   └── src/
│       ├── main.rs            # Native eframe application entry point
│       ├── app.rs             # egui UI, recording workflow, tray and shortcuts
│       ├── audio.rs           # Microphone capture and WAV encoding
│       ├── config.rs          # Preferences and credential handling
│       ├── focus_monitor.rs   # Best-effort active-field integration
│       ├── languages.rs       # Sarvam language names and codes
│       └── sarvam.rs          # Speech and translation API client
├── .github/workflows/
│   ├── ci.yml                 # Format, lint, tests and platform builds
│   └── release.yml            # Tagged multi-platform draft releases
└── README.md
```

## Sarvam API

- Base URL: `https://api.sarvam.ai`
- Speech-to-text endpoint: `POST /speech-to-text` (`saaras:v3` or `saaras:v4`)
- Translation endpoint: `POST /translate` (`mayura:v1`)
- The settings check exercises Mayura translation access only; it does not establish Saaras speech-recognition access.
- Recordings longer than 27 seconds are recognized and (when enabled) translated as separate segments and joined in order.
- Availability, quotas, accepted languages and modes are controlled by Sarvam and may change.

## Contributing

Issues and pull requests are welcome. Please include the affected operating system and desktop session when reporting shortcut, focus, microphone or text-insertion problems. Run the Rust formatting, lint and test commands above before opening a pull request.

## License

MIT — see [LICENSE](./LICENSE).
