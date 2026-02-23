# वाणी — Vaani

> **Speak any Indian language. Type anywhere.**

Vaani is a free, open-source, MIT-licensed desktop app that puts a small bubble near any text field you click. Press the bubble, speak in any of 22 Indian languages (or code-mixed Hinglish/Tanglish/etc.), and the transcribed — optionally translated — text is typed into whatever you were focused on.

Built on [Sarvam AI's Saaras v3 STT API](https://www.sarvam.ai/apis/speech-to-text/). You bring your own API key. No servers, no telemetry, no accounts.

---

## Features

- 🎙️ **22 Indian languages** — Hindi, Tamil, Telugu, Kannada, Malayalam, Bengali, Gujarati, Marathi, Punjabi, Odia, and more
- 🔀 **Code-mixed speech** — Hinglish, Tanglish, Manglish natively understood
- 🌐 **Translate to any language** — Set a destination language; Vaani transcribes then translates using Sarvam's Mayura API
- 🫧 **Smart bubble** — Appears near active text fields automatically; draggable to any position
- ⌨️ **Global hotkey** — Ctrl+Shift+Space (customisable) as an alternative trigger
- 🔒 **Your API key, your data** — Calls go directly from your machine to Sarvam's servers
- 🪶 **Tiny footprint** — Rust + Tauri, ~8MB binary

---

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+
- [Tauri CLI](https://tauri.app/v2/guides/getting-started/prerequisites/): `cargo install tauri-cli`
- A [Sarvam AI API key](https://www.sarvam.ai/) (free tier gives ₹1000 credits)

### macOS: Accessibility Permission

Vaani needs Accessibility access to detect which app is in focus and simulate keyboard paste.

1. Build and run the app once
2. Open **System Settings → Privacy & Security → Accessibility**
3. Enable **Vaani**

### Windows

No extra steps required for basic functionality.

---

## Getting Started

```bash
git clone https://github.com/your-username/vaani
cd vaani
npm install
cargo tauri dev
```

To build a release binary:
```bash
cargo tauri build
```

---

## Usage

1. Launch Vaani — a small orange bubble appears in the bottom-right corner
2. Click any text field in any app (Gmail, VS Code, Slack, WhatsApp Web, Notes…)
3. The bubble moves near the text field and glows
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
- Bubble position / always-on-top behaviour

---

## How It Works

```
[Microphone] → cpal (Rust audio capture)
    → WAV buffer in memory
    → POST /speech-to-text (Sarvam Saaras v3)
    → transcript + detected language
    → (if target_lang ≠ source_lang) POST /translate (Sarvam Mayura v1)
    → final text → clipboard → simulate Cmd+V / Ctrl+V
```

---

## Sarvam API Reference

| API | Endpoint | Used for |
|-----|----------|----------|
| Speech-to-Text | `POST https://api.sarvam.ai/speech-to-text` | Transcription |
| Translate | `POST https://api.sarvam.ai/translate` | Language translation |

Pricing (Feb 2026): STT at ₹30/hour of audio. For typical use (~30 min/day), cost is ~₹15/month.

---

## Supported Languages

| Code | Language | Code | Language |
|------|----------|------|----------|
| `hi-IN` | Hindi | `ta-IN` | Tamil |
| `te-IN` | Telugu | `kn-IN` | Kannada |
| `ml-IN` | Malayalam | `bn-IN` | Bengali |
| `gu-IN` | Gujarati | `mr-IN` | Marathi |
| `pa-IN` | Punjabi | `or-IN` | Odia |
| `en-IN` | English (Indian) | `auto` | Auto-detect |

---

## Contributing

PRs welcome! Key areas that need work:
- Windows text-field detection (UI Automation API)
- Linux support (AT-SPI)
- Custom vocabulary / personal dictionary sync
- Waveform visualisation improvements

---

## License

MIT — see [LICENSE](./LICENSE)
