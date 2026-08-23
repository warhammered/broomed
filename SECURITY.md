# Security Policy

## Architecture

Broomed is a local-first desktop app. The trusted boundary is the local filesystem; no file contents leave the machine unless you explicitly configure a cloud AI provider.

Key primitives in `crates/broomed-core`:

- **`security::validate_path`** (`crates/broomed-core/src/security.rs`) — rejects empty/NUL paths, `..` components, and paths escaping the canonicalized `base`. All planned operations (`operation::plan_move`) go through it.
- **`fs::SafeWalk` / `TraversalBudget`** (`crates/broomed-core/src/fs.rs`) — budgeted walk with `follow_symlinks: false` and `max_files`/`max_depth` caps. Symlinks are not followed; hidden files excluded by default.
- **`operation::execute` / `Journal`** (`crates/broomed-core/src/operation.rs`) — atomic `rename` on same device, cross-device fallback is `copy → hash-verify (BLAKE3) → remove source`. Destination must not exist; undo reverses the same checks and marks `status = 'undone'` in SQLite.
- **`hash`** (`crates/broomed-core/src/hash.rs`) — BLAKE3 hashing for post-copy verification.
- **`watcher::WatchConfig`** (`crates/broomed-core/src/watcher.rs`) — `should_ignore` enforces `excluded` prefixes and restricts events to `watched` roots when configured.

The Tauri shell (`src-tauri/src/main.rs`) exposes only narrow commands (`scan_directory`, `hash_file`, `parse_intent`, `mascot_state`) and performs no filesystem writes itself.

No Python-specific hardening remains — the legacy stack was removed.

## Reporting a Vulnerability

Do **not** open a public issue for sensitive reports.

Use **GitHub Security Advisories** (private) on this repository:

https://github.com/warhammered/broomed/security/advisories/new

Include:

- Vulnerability type (path traversal, symlink escape, hash bypass, etc.)
- OS, Rust version (`rustc --version`), and Broomed version (`0.1.0`)
- Reproduction steps and proof-of-concept if available
- Impact assessment

You should receive an acknowledgment within 48 hours. We aim to triage within 7–14 days and will credit you in the advisory if you wish.

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |
| < 0.1   | No        |

Only the current `0.1.0` line (Rust/Tauri) is supported. The legacy `2.x` Python line is unmaintained.

## Hardening Notes

- All `plan_move`/`execute` paths reject overwrites and verify with hashing for files.
- `validate_path` canonicalizes the base; callers must pass the intended root (not a symlink).
- Watcher debounce/ignore logic does not change safety invariants — operations still re-validate at execution.
- Environment keys (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`) are read from `.env` only; never logged.
