# Broomed — The local-first AI file organizer.

Broomed organizes messy directories into clean, explainable structures — locally, safely, and reversibly. A Rust core does the filesystem work; a Tauri shell provides the native desktop UI.

> **Version:** 0.1.0 · **License:** MIT OR Apache-2.0 · **Platforms:** Windows, macOS, Linux

## What Broomed Does

- **Scans** a source directory with a budgeted, symlink-safe walk.
- **Classifies** files by content, name, and context (local inference or optional cloud provider).
- **Plans** moves/copies/renames as explicit, previewable operations — never implicit bulk renames.
- **Executes** atomically with hash verification and a journaled history so every change can be undone.
- **Learns** lightweight preferences without exfiltrating file contents.

Typical flow: pick a source folder → preview the plan → execute → undo if needed. No daemon required, no cloud required.

## Core Principles

- **Local-first** — works offline; network is opt-in per provider.
- **Privacy by default** — file contents stay on device; cloud providers only used when you configure them.
- **Safe filesystem ops** — path validation, symlink protection, protected-directory checks, bounded traversal.
- **Explainable** — every operation has a reason and confidence; plans are inspectable before execution.
- **Reversible** — journaled history with undo; no overwrites.
- **Cross-platform** — Tauri bundle targets `all` (Windows, macOS, Linux).
- **AI-assisted, not dependent** — heuristics + hashing work without a model; AI improves suggestions.
- **Minimal desktop** — small native window, no Electron, no background services unless you run the watcher.

## Architecture

```
Tauri Desktop Shell (src-tauri)
  └─ Frontend / Mascot UI  (Tauri WebView — HTML/CSS/JS; mascot state via broomed-core::mascot)
       └─ Rust App Layer   (src-tauri/src/main.rs — Tauri commands bridging UI ↔ core)
            └─ Broomed Core (crates/broomed-core)
                 ├─ filesystem  — SafeWalk, TraversalBudget (crates/broomed-core/src/fs.rs)
                 ├─ indexing    — walk + metadata, hash (hash.rs, db.rs)
                 ├─ classification — intent parsing (intent.rs), AI task routing (ai.rs)
                 ├─ organization — plan_move / execute / copy_recursively (operation.rs)
                 ├─ safety      — validate_path, protected-dir checks (security.rs)
                 ├─ history     — SQLite journal, record / undo (operation.rs::Journal, db.rs)
                 └─ Embedded AI — hardware (hardware.rs), model manager (models.rs),
                                  engines (engines.rs: Embedding/Text/Vision/Audio/OCR/Media),
                                  orchestrator (orchestrator.rs), analysis (analysis.rs),
                                  cache (cache.rs), search (search.rs), watcher (watcher.rs)
```

Actual crate layout (`crates/broomed-core/src`): `ai`, `analysis`, `bridge`, `cache`, `db`, `engines`, `error`, `fs`, `hardware`, `hash`, `intent`, `mascot`, `models`, `operation`, `orchestrator`, `search`, `security`, `types`, `watcher`.

Data flow: `SafeWalk` → `hash`/`fs` metadata → `orchestrator::Orchestrator::analyze` (deterministic metadata → decides minimal AI specialists → invoke only needed engine) → `engines` (Embedding/TextReasoning/Vision/Audio/OCR/Media, all lazily loaded, CPU-only fallback) → `analysis::FileAnalysis` (normalized, hash+model-version cache key) → `operation::plan_move` (validates src/dst under base, rejects existing dst) → `operation::execute` (rename with cross-device copy+hash-verify fallback, `create_dir_all` for parents) → `Journal::record` → `Journal::undo` (reverse rename with same verification).

## AI (Embedded, Offline)

Broomed is an AI application, not an AI client. No Ollama, LM Studio, Python, Docker, or local server is required.

```
Install Broomed → Launch → Hardware detection → Embedded AI ready → Scan & organize
```

- **Embedded by default** — tiny quantized models managed by Broomed itself (`models.rs`): versioned manifests, BLAKE3 checksums, atomic updates, lazy loading, idle unload, corruption recovery. Models live separate from the binary so app updates ≠ model updates. Default payload target ~500–700 MB, no single model >300 MB.
- **Model set** (initial candidates):
  - `all-MiniLM-L6-v2` (384-dim, ~80 MB) — semantic embeddings (ONNX/candle) for search, similarity, duplicate & related-file discovery. Abstracted so another small encoder can replace it.
  - `SmolLM2-360M-Instruct` (quantized Q4_K_M, ~220 MB) — structured classification/tags/reason via llama.cpp/GGUF, tiny reasoning engine.
  - `SmolVLM2-256M-Video-Instruct` (~180 MB incl. projector) — image/video keyframe understanding; 500M kept as optional quality trade-off, not shipped by default.
  - `Whisper tiny` (~75 MB) — audio transcription, invoked only when ID3/metadata insufficient.
  - Tesseract (or small permissive OCR) + embedded FFmpeg — deterministic OCR/media probing, not LLM work.
- **Orchestrated** (`orchestrator.rs`) — metadata/type/hash → deterministic analysis → decide minimal specialists (e.g. MP3 with ID3 skips Whisper, TXT skips vision/OCR, scanned PDF triggers OCR, video samples keyframes). Bounded concurrency, streaming extraction, hash+model-version cache.
- **Provider abstraction** (`ai.rs`, `types::ProviderId`, `engines.rs`) keeps AI optional: `EmbeddingEngine`/`TextReasoningEngine`/`VisionEngine`/`AudioTranscriptionEngine`/`OcrEngine`/`MediaEngine` with heuristic stubs that work offline with zero models. Real GGUF/ONNX/whisper.cpp behind `local-ai` feature, lazily loaded.
- **Optional cloud** (explicit opt-in, separate path) — set `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` in `.env` only if you opt in. Default path never contacts network, works fully offline after models cached. No telemetry, no file-content upload.
- **Hardware tiers** (`hardware.rs`) — Tier0 (CPU-only, small models, concurrency 2) / Tier1 (CPU+accel) / Tier2 (GPU batching). CPU-only always works; GPU is opportunistic.
- **Safety** — AI never touches filesystem. `AI → suggestion → validated operation plan → user preview → filesystem executor` with `security::validate_path`, protected-dir checks, and journaled undo authoritative.

## Safety

- **Preview / dry-run** — `plan_move` validates before any I/O; UI previews the full operation list.
- **Atomic moves** — `std::fs::rename` on same device; cross-device falls back to copy → hash-verify (BLAKE3/SHA) → remove source.
- **No overwrites** — `plan_move` and `execute` reject if destination exists; undone operations check the same.
- **Path validation** (`security::validate_path`) — rejects `..`, absolute escapes, empty/NUL paths; base is `canonicalize`d first.
- **Symlink protection** — `SafeWalk` (`follow_symlinks: false` by default), no symlink following in `copy_recursively`; traversal budget caps `max_files`/`max_depth`.
- **Protected directories** — watcher/config `excluded` list honored; `WatchConfig::should_ignore` skips excluded prefixes.
- **Rollback** — `Journal` stores every operation in SQLite (`crates/broomed-core/migrations`); `undo` reverses dst→src with parent recreation and hash check, then marks `status = 'undone'`.

## Installation

Prebuilt bundles are produced by Tauri (`src-tauri/tauri.conf.json` → `bundle.targets = "all"`):

| Platform | Artifact (from `tauri build`) |
|----------|-------------------------------|
| Windows  | `.msi` / `.exe` (NSIS)       |
| macOS    | `.app` / `.dmg`              |
| Linux    | `.deb` / `.AppImage`         |

Until releases are published, run from source (see Development). No Python, Docker, or `pip` required.

## Development

### Prerequisites

- **Rust** stable toolchain (`rustup` + `cargo`) — required.
- **Tauri prerequisites** per OS: WebView2 (Windows, usually present), WebKitGTK + dependencies on Linux, Xcode CLT on macOS. See [Tauri prerequisites](https://tauri.app/start/prerequisites/).
- **Node / npm** — only if you add a frontend bundler; current `src-tauri` has no `beforeDevCommand`/`beforeBuildCommand` (pure Rust shell).

### Working with the core

```bash
# Check and test the core crate
cargo check -p broomed-core
cargo test -p broomed-core
cargo clippy -p broomed-core -- -D warnings

# Or from the crate directory
cargo check --manifest-path crates/broomed-core/Cargo.toml
cargo test  --manifest-path crates/broomed-core/Cargo.toml
```

### Working with the desktop shell

```bash
# Install Tauri CLI if needed
cargo install tauri-cli --locked

# Dev run (opens the Tauri window)
cargo tauri dev --manifest-path src-tauri/Cargo.toml
# or: cargo run --manifest-path src-tauri/Cargo.toml  (without Tauri window chrome)

# Production bundle (writes to src-tauri/target/release/bundle/)
cargo tauri build --manifest-path src-tauri/Cargo.toml
```

### Environment

Embedded AI needs no configuration — launch and scan. For optional cloud or testing overrides:

```bash
cp .env.example .env
# edit .env — set OPENAI_API_KEY / ANTHROPIC_API_KEY only if you opt into cloud
# BROOMED_MODEL_DIR, BROOMED_FORCE_GPU are for testing only
```

No Ollama, Python, Docker, or model-server configuration required.

## Project Structure

```
broomed/
├── crates/
│   └── broomed-core/          # Rust core library (safe FS, indexing, ops, AI, history)
│       ├── Cargo.toml         # version 0.1.0
│       ├── migrations/        # SQLite schema (0001_files_index.sql)
│       └── src/
│           ├── lib.rs         # crate root — re-exports modules
│           ├── ai.rs          # provider abstraction, AiTask / AiCapabilities (now embeds local)
│           ├── analysis.rs    # FileAnalysis normalized multimodal representation
│           ├── bridge.rs      # Tauri-friendly wrappers (scan, hash, intent, mascot)
│           ├── cache.rs       # analysis cache (hash+model-version) + SQLite persistence
│           ├── db.rs          # SQLite helpers, create_schema
│           ├── engines.rs     # Embedding/Text/Vision/Audio/OCR/Media traits + heuristic stubs
│           ├── error.rs       # CoreError
│           ├── fs.rs          # SafeWalk, TraversalBudget
│           ├── hardware.rs    # hardware tier detection (Tier0/Tier1/Tier2)
│           ├── hash.rs        # file hashing (BLAKE3)
│           ├── intent.rs      # natural-language intent parsing
│           ├── mascot.rs      # MascotState (UI hint)
│           ├── models.rs      # model manager (manifest, checksum, atomic install, lazy)
│           ├── operation.rs   # OpKind, Operation, plan_move, execute, Journal
│           ├── orchestrator.rs# intelligent routing - minimal required specialists
│           ├── search.rs      # semantic search (LIKE + embedding hybrid)
│           ├── security.rs    # validate_path
│           ├── types.rs       # FileId, OperationId, ProviderId, DirectoryId
│           └── watcher.rs     # WatchConfig, debounce/ignore logic
├── src-tauri/                 # Tauri desktop shell
│   ├── Cargo.toml             # version 0.1.0, productName Broomed
│   ├── tauri.conf.json        # identifier com.broomed.desktop, bundle targets all
│   └── src/main.rs            # Tauri commands: scan_directory, hash_file, parse_intent, mascot_state
├── .github/
│   ├── workflows/             # ci.yml, release.yml, security.yml
│   └── accepted-risks.yml
├── .env.example               # minimal — embedded AI (no Ollama) + optional cloud keys
├── .gitignore                 # Rust/Tauri/Node/OS ignores
├── renovate.json              # cargo / npm / github-actions
├── LICENSE                    # MIT
├── SECURITY.md
├── CONTRIBUTING.md
└── CODE_OF_CONDUCT.md
```

No `docs/` directory — legacy web/TUI docs were removed. No Python packaging remains.

## Contributing

Keep changes small and explain the why.

1. Fork, branch from `main`, make a focused commit.
2. Run `cargo check`, `cargo test`, `cargo clippy -- -D warnings` (core) and `cargo check --manifest-path src-tauri/Cargo.toml` if you touch the shell.
3. If you add a frontend, include `npm ci` / `npm run build` as needed and keep `src-tauri/tauri.conf.json` bundle targets accurate.
4. Open a PR with a clear description and test evidence.

See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## Security

For architecture, threat model, and hardening notes see [SECURITY.md](SECURITY.md). To report a vulnerability, open a private GitHub Security Advisory on this repository. Do not file public issues for sensitive reports.

## License

MIT — see [LICENSE](LICENSE). Apache-2.0 dual-license text is available on request; current `LICENSE` file is MIT and `src-tauri/Cargo.toml` declares `MIT OR Apache-2.0`.
