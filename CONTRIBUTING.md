# Contributing to VaaniFlow

Thank you for your interest in contributing. VaaniFlow is a native Rust desktop app with an egui interface for speech recognition and translation using Sarvam APIs. Cross-application focus and input behavior varies by operating system; see [Platform status](README.md#platform-status) before working on platform integrations.

## Project governance

VaaniFlow is community-friendly and maintained by `alpha912`:

- Contributions are welcome via fork and pull request.
- Changes to `main` go through review and maintainer approval.
- Releases are managed by the maintainer.

## Platform status

| Platform | Current behavior | Areas to improve |
| :--- | :--- | :--- |
| **Windows** | Best-effort focus detection for common text controls and best-effort paste to the last detected target. | Broaden detection and test foreground restoration across editors and browsers. |
| **macOS** | Native UI/audio build target; hotkey-first workflow and manual clipboard insertion. | Accessibility-based focus detection and tested text insertion. |
| **Linux** | Native UI/audio build target; hotkey-first workflow and manual clipboard insertion. Global hotkeys depend on X11; Wayland behavior depends on the compositor. | AT-SPI integration, Wayland-friendly shortcut/input support and distro testing. |

Build success on a platform does not mean every cross-app integration is complete there. Please report your OS version, desktop environment/session, and the exact behavior when filing platform issues.

## Development setup

### Prerequisites

- Rust stable via [rustup](https://rustup.rs/).
- A [Sarvam AI API key](https://www.sarvam.ai/) for live transcription and translation tests.
- Platform toolchains and audio/window-system development libraries. See [README build prerequisites](README.md#prerequisites).

### Getting started

1. Fork and clone the repository:
   ```bash
   git clone https://github.com/<your-username>/VaaniFlow.git
   cd VaaniFlow
   ```
2. Run the native app:
   ```bash
   cargo run -p vaaniflow
   ```
3. Enter your API key in the app. It is stored in the OS credential store where available; do not add credentials to source files, logs, or pull requests.

Run the local checks before submitting changes:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build -p vaaniflow --release --locked
```

## Making changes

### Branch strategy

- Create a feature branch from an up-to-date `main`, e.g. `git checkout -b feat/your-feature`.
- Use descriptive prefixes such as `feat/`, `fix/`, `docs/`, or `refactor/`.

### Code style

- Run `cargo fmt --all` before committing Rust changes.
- Run Clippy and address warnings.
- Keep UI changes in `native/src/app.rs`; API, audio, configuration, language and platform integration code live in the adjacent modules.
- Avoid logging API credentials, transcripts, request/response bodies, or recorded audio.
- Add or update tests for behavior that can be exercised without a microphone, network account, or desktop session.

### Commit messages

- Use imperative mood: “Add feature”, not “Added feature”.
- Keep the subject line under 72 characters.
- Reference issues where applicable, e.g. “Fix #42: handle empty audio buffer”.

## Pull request process

1. Check whether the change fits an existing issue; open an issue first for a substantial new feature.
2. For larger changes, open a draft PR early to align on the approach.
3. Rebase on the latest `main` before requesting review.
4. Run the local checks above and describe which passed. For platform work, include OS, desktop session and manual testing details.
5. Open a PR against `main` and include the problem, solution, tests, relevant UI screenshots or logs, and linked issues.
6. CI runs Rust formatting, Clippy, tests and platform builds. Address failures before requesting review.
7. One maintainer approval is required before merge.

Keep PRs focused. Rebase rather than merge commits where practical; squash merges are preferred.

## Areas that need work

- macOS accessibility-based focus detection and tested text insertion.
- Linux AT-SPI and desktop/compositor integration, especially Wayland.
- More reliable cross-application paste and platform permission guidance.
- Integration tests for settings migration, API request construction, and desktop interactions.
- Streaming recognition, richer recording feedback, and improved vocabulary workflows.
- Signed installers and platform-specific packaging.

## Security

- Never commit API keys or user transcripts.
- Release builds do not contain an API key; each user supplies their own.
- Report vulnerabilities privately via [SECURITY.md](./SECURITY.md).

## Releases

- Releases are managed by `alpha912`.
- Do not add release-only commits to feature PRs.
- Version tags must match the package version in `native/Cargo.toml`. The tag workflow builds unsigned portable archives and creates a draft release for maintainer review.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](./LICENSE).
