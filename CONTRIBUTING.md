# Contributing to Broomed

Thanks for helping improve Broomed.

## Prerequisites

- Rust stable via `rustup` (`cargo`, `rustc`, `clippy`).
- Tauri system deps for your OS: WebView2 on Windows, WebKitGTK on Linux, Xcode CLT on macOS. See https://tauri.app/start/prerequisites/.
- Optional: Node/npm only if you work on a frontend bundler (current shell has no build step).

No Python, `pip`, `uv`, or Docker required.

## Setup

```bash
git clone https://github.com/warhammered/broomed.git
cd broomed

# Core crate
cargo check -p broomed-core
cargo test  -p broomed-core

# Desktop shell
cargo check --manifest-path src-tauri/Cargo.toml
```

Copy `.env.example` to `.env` only if you need AI providers:

```bash
cp .env.example .env
# set OLLAMA_HOST, OLLAMA_MODEL, OPENAI_API_KEY, ANTHROPIC_API_KEY as needed
```

## Workflow

1. Branch from `main`: `git checkout -b feature/short-name`
2. Make a focused change with tests where behavior is involved.
3. Run checks before pushing:

```bash
cargo check -p broomed-core
cargo test  -p broomed-core
cargo clippy -p broomed-core -- -D warnings

# if you touched the shell
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings

# optional full run
cargo test --workspace
```

4. Commit with a conventional prefix (`feat:`, `fix:`, `docs:`, `chore:`) and a clear message.
5. Open a PR against `main` with context, testing, and screenshots when UI changes.

## Code Style

- `cargo fmt` (rustfmt) — format before pushing.
- `cargo clippy -- -D warnings` — no warnings.
- Keep modules small; prefer `crates/broomed-core/src/*.rs` conventions already in the repo.
- Safety-critical code (`fs.rs`, `security.rs`, `operation.rs`) needs explicit tests for path validation, no-overwrite, and hash verification.

## Project Map

- `crates/broomed-core/` — filesystem safety, hashing, operations, journal, AI abstraction.
- `src-tauri/` — Tauri shell, commands in `src/main.rs`, config in `tauri.conf.json`.
- `.github/workflows/` — CI, release, security scans.

## Pull Requests

- One concern per PR.
- Include `cargo test` output or CI link.
- Update `README.md` only when behavior or setup actually changes.

## Questions

Open a GitHub issue or discussion. For security-sensitive reports, use a private Security Advisory instead of a public issue.
