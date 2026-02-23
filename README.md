# वाणीFlow — VaaniFlow

> **Speak any Indian language. Type anywhere.**

VaaniFlow is a free, open-source, MIT-licensed desktop app that puts a small bubble near any text field you click. Press the bubble, speak in any of 22+ Indian languages (or code-mixed Hinglish/Tanglish/etc.), and the transcribed — optionally translated — text is typed into whatever you were focused on.

Built on [Sarvam AI's Saaras v3 STT API](https://www.sarvam.ai/). You bring your own API key. No servers, no telemetry, no accounts.

---

## Features

- 🎙️ **22+ Indian languages** — Hindi, Tamil, Telugu, Kannada, Malayalam, Bengali, Gujarati, Marathi, Punjabi, Odia, Assamese, Urdu, Nepali, and more
- 🔀 **Code-mixed speech** — Hinglish, Tanglish, Manglish natively understood
- 🌐 **Translate to any language** — Set a destination language; VaaniFlow transcribes then translates using Sarvam's Mayura API
- 🫧 **Smart bubble** — Appears near active text fields automatically; draggable to any position
- ⌨️ **Global hotkey** — Ctrl+Shift+Space (customisable) as an alternative trigger
- 🔒 **Your API key, your data** — Calls go directly from your machine to Sarvam's servers
- 🪶 **Tiny footprint** — Rust + Tauri v2, ~8MB binary

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Frontend | React 19, TypeScript 5.8, Vite 7 |
| Backend | Rust (2021 edition), Tauri v2 |
| Audio | cpal (capture), hound (WAV encoding) |
| API | Sarvam Saaras v3 (STT), Sarvam Mayura v1 (Translation) |
| Platform | Win32 APIs (Windows), AppleScript (macOS) |

---

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+ (recommended: 24+)
- [Tauri CLI](https://tauri.app/): `cargo install tauri-cli`
- A [Sarvam AI API key](https://www.sarvam.ai/) (free tier gives ₹1,000 credits)

### macOS: Accessibility Permission

VaaniFlow needs Accessibility access to detect which app is in focus and simulate keyboard paste.

1. Build and run the app once
2. Open **System Settings → Privacy & Security → Accessibility**
3. Enable **VaaniFlow**

### Windows

No extra steps required for basic functionality.

---

## Getting Started

```bash
git clone https://github.com/alpha912/VaaniFlow.git
cd VaaniFlow
npm install
cargo tauri dev
```

Create a `.env.development` file with your API key:
```
VITE_SARVAM_API_KEY=your_api_key_here
```

To build a release binary:
```bash
cargo tauri build
```

---

## Usage

1. Launch VaaniFlow — a small orange bubble appears on screen
2. Click any text field in any app (Gmail, VS Code, Slack, WhatsApp Web, Notes…)
3. The bubble glows to indicate it detected the text field
4. **Click the bubble** or press **Ctrl+Shift+Space** to start recording
5. Speak naturally in any language
6. Click the bubble again (or press the hotkey) to stop
7. Text is transcribed (and translated if you set a target language) and typed into your text field

### Settings

Right-click the bubble → **Settings** to configure:
- Sarvam API key
- Source language (or `auto` for auto-detection)
- Destination language (set to `Same as source` to skip translation)
- Global hotkey

---

## How It Works

```
[Microphone] → cpal (Rust audio capture)
    → WAV buffer in memory (16-bit PCM, 16kHz mono)
    → POST /speech-to-text (Sarvam Saaras v3)
    → transcript + detected language
    → (if target_lang ≠ source_lang) POST /translate (Sarvam Mayura v1)
    → final text → clipboard → simulate Cmd+V / Ctrl+V
```

---

## Sarvam API Reference

| API | Endpoint | Model | Used for |
|-----|----------|-------|----------|
| Speech-to-Text | `POST https://api.sarvam.ai/speech-to-text` | saaras:v3 | Transcription |
| Translate | `POST https://api.sarvam.ai/translate` | mayura:v1 | Language translation |

Supported audio formats: WAV, MP3, AAC, AIFF, OGG, OPUS, FLAC, MP4/M4A, AMR, WMA, WebM, PCM

**Note:** STT requests are limited to 30 seconds per call.

---

## Supported Languages

| Code | Language | Code | Language |
|------|----------|------|----------|
| `hi-IN` | Hindi | `ta-IN` | Tamil |
| `te-IN` | Telugu | `kn-IN` | Kannada |
| `ml-IN` | Malayalam | `bn-IN` | Bengali |
| `gu-IN` | Gujarati | `mr-IN` | Marathi |
| `pa-IN` | Punjabi | `or-IN` | Odia |
| `en-IN` | English (Indian) | `as-IN` | Assamese |
| `ur-IN` | Urdu | `ne-IN` | Nepali |
| `auto` | Auto-detect | | |

---

## Contributing

PRs welcome! Key areas that need work:
- Linux support (AT-SPI2 text-field detection)
- WebSocket streaming STT for real-time transcription
- Custom vocabulary / personal dictionary sync
- Waveform visualisation improvements

---

## License

MIT — see [LICENSE](./LICENSE)
