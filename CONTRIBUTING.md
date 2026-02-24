# Contributing to VaaniFlow

Thank you for your interest in contributing! VaaniFlow is a Tauri 2 desktop app
that brings speech-to-text for 22+ Indian languages to any text field.

## Platform Status

| Platform | Status | Notes |
| :------- | :----- | :---- |
| **Windows** | Supported | Primary development platform |
| **macOS** | Needs contributors | AppleScript focus detection scaffolded but untested |
| **Linux** | Planned | AT-SPI2 text-field detection needed |

**macOS developers are especially welcome!** If you have a Mac and want to help
bring VaaniFlow to macOS, please open an issue to coordinate.

## Development Setup

### Prerequisites

| Tool | Version | Link |
| :--- | :------ | :--- |
| Rust | stable | [rustup.rs](https://rustup.rs/) |
| Node.js | 18+ | [nodejs.org](https://nodejs.org/) |
| Sarvam API Key | free tier | [sarvam.ai](https://www.sarvam.ai/) |

### Getting Started

1. Fork and clone the repository:
   ```bash
   git clone https://github.com/<your-username>/VaaniFlow.git
   cd VaaniFlow
   ```

2. Install frontend dependencies:
   ```bash
   npm install
   ```

3. Create your environment file:
   ```bash
   cp .env.example .env.development
   # Edit .env.development and add your Sarvam API key
   ```

4. Run in development mode:
   ```bash
   cargo tauri dev
   ```

5. Build for production:
   ```bash
   cargo tauri build
   ```

## Making Changes

### Branch Strategy

- Create a feature branch from `main`: `git checkout -b feat/your-feature`
- Use descriptive branch prefixes: `feat/`, `fix/`, `docs/`, `refactor/`

### Code Style

**Rust:**
- Run `cargo fmt` before committing
- Run `cargo clippy` and address all warnings
- Follow existing patterns in `src-tauri/src/`

**TypeScript / React:**
- TypeScript strict mode is enabled (see `tsconfig.json`)
- No unused locals or parameters
- Follow existing component patterns in `src/`

### Commit Messages

- Use imperative mood: "Add feature" not "Added feature"
- Keep the subject line under 72 characters
- Reference issues where applicable: "Fix #42: handle empty audio buffer"

## Pull Request Process

1. Ensure your branch is up to date with `main`
2. Verify all checks pass locally:
   ```bash
   npm run build           # Frontend builds
   cargo fmt --check       # Rust formatting
   cargo clippy            # Rust lints
   cargo check             # Rust compilation
   ```
3. Open a PR against `main` with a clear description of the change
4. CI will run automatically on your PR
5. Address any review feedback

## Areas That Need Work

- **macOS support** — Accessibility permission, AppleScript focus detection
- **Linux support** — AT-SPI2 text-field detection
- **Streaming STT** — WebSocket-based real-time transcription
- **Custom vocabulary** — Personal dictionary sync
- **Waveform visualisation** — Better recording feedback
- **Tests** — Unit and integration test coverage

## Security

- **Never commit API keys.** The `.env.development` file is gitignored.
- Release builds do not bake in any API key; users provide their own.
- Report security vulnerabilities privately to alphinctom@gmail.com.

## License

By contributing, you agree that your contributions will be licensed under the
[MIT License](./LICENSE).
