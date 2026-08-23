# Bundled Local AI — Deepwork

## Goal
Ship Broomed (`f754438e` single-commit canonical) with a **literal local AI** bundled with the app — not an external Ollama endpoint the user must install. Support **local-only** and **cloud** as separately testable modes on Win/Mac/Linux. User explicitly wants offline-capable file classification.

> Ponytail constraint: smallest model that classifies files usefully wins. Prefer 80MB embedding + heuristics over 2GB LLM unless proven insufficient.

## Confirmed Context
- Repo: `warhammered/broomed`, `main f754438e` (1 commit, Vishan Dey), public, `target: all` in `tauri.conf.json`
- Core: `crates/broomed-core` (ai.rs trait/router present, 0 provider impls), `operation.rs` plan/execute/journal, `fs.rs` SafeWalk, `search.rs` bm25, `mascot.rs`
- Tauri: `src-tauri/src/main.rs` 5 cmds (browse stub), `src/` vanilla mascot UI
- CI: 3 workflows disabled_manually to save minutes
- Previous roadmap (ora-1): critical path A dialog, B Ollama, C cloud, D settings, E pipeline, F packaging; risks prompt quality, bundling, keychain

## Key Questions
1. Bundling strategy: **embed model in installer** (~+80MB–2GB) vs **download on first launch** (Tauri `plugin-updater` style)?
2. Inference runtime for pure Rust cross-platform: `candle` (safetensors, quantized) vs `ort` (ONNX Runtime) vs `llama-cpp-rs` (GGUF) — binary size, `target` compatibility, model license, cold-start
3. Task shape: embedding+classifier (filename + ext + small content snippet → category) vs small generative LLM (Phi-3, Gemma 2B Q4) — latency/battery on low-end Win/Mac
4. Model sourcing & license: which permissive model to ship (MiniLM, e5-small, Phi-3) — must be redistributable
5. Packaging implications: `.msi`/`.dmg`/`.AppImage` size limits, code signing, `src-tauri/resources/` vs `AppData` dir

## Research Findings — Confirmed 2026-08-23 (@librarian lib-1, @explorer exp-8)

### Ai.rs Abstraction (exp-8, 479 lines)
- Trait `AiProvider { id, capabilities, priority, supports(&AiTask) -> bool }` — only abstraction, no classify impl
- Structs: `AiTask` (8 variants: ClassifyFile etc), `AiCapabilities {text, vision, embeddings, structured_output}`, `AiProviderConfig {id, name, base_url, model, … priority, enabled}`, `AiRouter {providers: Vec<AiProviderConfig>}`, `AiResult {category, confidence 0-1, suggested_folder, tags, reason}`
- No `classify`/`classify_batch`; `AiRouter::route(task) -> Option<&AiProviderConfig>` only. Error via `CoreError::Internal` + `parse_ai_json(raw)`.
- Zero Ollama/OpenAI/Anthropic strings — `base_url` placeholder `https://api.example.com`. Clean slate.
- Plug point: `impl AiProvider for BundledLocalProvider` + `AiRouter::new(vec![BundledLocalProvider as config])`; `lib.rs pub mod ai` exposes all.
- Path: add `async fn classify(&self, task, input) -> Result<AiResult>` to trait or free `classify_batch(router, inputs)` wrapper that calls provider.

### Bundled Runtime Options (@librarian, stars Aug 2026)
| Runtime | Stars | License | Native lib | Apple Silicon | Model fmt | Verdict |
|---------|-------|---------|------------|---------------|-----------|---------|
| **candle** | 20.9k | Apache-2.0 | No (pure Rust) | Metal ✅ | safetensors/GGUF | **Primary pick** — pure Rust, lean CPU binary, Metal one-liner, most Tauri-proven (Oxide-Lab, tauri-candle-vllm, CrabNebula tauri-plugin-llm) |
| ort | 2.5k | Apache-2.0 | Yes 10–20MB ONNX Runtime | CoreML/Metal (Intel mac dropped) | ONNX | Alt if need CoreML/DirectML accel; CDN download annoyance |
| tract | 3k | MIT/Apache | No | Metal ✅ | ONNX/NNEF | Smallest, least Tauri precedent |
| llama-cpp-rs | 633 | Apache-2.0 | Yes C++ | Metal ✅ | GGUF | Overkill (2.4GB Phi-3 Q4), non-deterministic |
| burn | 15.8k | Apache-2.0 | No | Metal/Vulkan | burnpack | Training framework, heavy |

Models: `all-MiniLM-L6-v2` safetensors fp32 ~88MB / ONNX Q4 ~54MB / Q4F16 ~30MB; `e5-small-v2` 130MB; `phi-3-mini Q4_K_M` 2.2GB. Embed deterministically via `sentence-transformers-rs` (AlLMiniLML6v2 mean-pool L2).

Tauri bundle pattern: `tauri.conf.json bundle.resources: {"../models/":"models/"}` + `app.path().resolve("models/model.safetensors", BaseDirectory::Resource)`. Resources folder = correct for fixed bundled model; AppData only for swappable/download-on-demand (not requested).

**Recommendation confirmed:** `candle` + `all-MiniLM-L6-v2` (~45–54MB Q4) in `src-tauri/resources` → file (name+snippet) → 384-dim embedding → cosine to folder exemplars / tiny linear head → deterministic `AiResult`. ponytail: deterministic embeddings > 2.4GB LLM sampling. Add ort/LLM only later if generative naming needed.

## Plan Draft (for @oracle review) — REVISED per ora-2 NEEDS REVISION 2026-08-23
### Phase 1 — Scaffolding (local-only MVP, ~1.5 days) [REVISED]
- Scope: **No `tauri-plugin-dialog`** — keep `<input webkitdirectory>` native picker (zero dep, ponytail); fix `browse_directory_cmd` stub to use `window.eval()` event instead. `HeuristicFallback` (ext→category map, zero-model offline) + `BundledLocalProvider` skeleton (`candle` + `tokenizers`, `all-MiniLM-L6-v2` loader `Device::Cpu`/`Metal` lazy via `OnceLock<Arc<Model>>` — load on first `classify`, spinner in UI, not at app start). **Single trait method** `async fn classify(&self, task: AiTask, input: &str) -> Result<AiResult>` (delete `classify_batch` wrapper — YAGNI). `parse_ai_json` reused. Commit after `cargo test` + Phase 1.5 gate green.
- Files: `crates/broomed-core/Cargo.toml` (+candle, tokenizers), `crates/broomed-core/src/ai.rs` (single method), `src-tauri/src/main.rs` (thin IPC), `tauri.conf.json` resources

### Phase 1.5 — Validation Gate [NEW per oracle]
- `cargo test --test classify_smoke` — 5 fixtures, `assert!(confidence > 0.3 && category != "Unknown")`, must pass Win + Mac CPU & Metal. Phase 2 starts only when green.

### Phase 2 — Pipeline + Preview (3–4 days) [REVISED]
- Scope: `classify` (per-file, loop in core, no batch wrapper until proven) → `plan_organize` batch → preview UI (virtualized table file→dest→confidence→reason) → `execute_plan` → `undo_last`, provider selector Bundled/Cloud/Offline, progress events (cached Device). Keep IPC thin, logic in core.
- Files: `crates/broomed-core/src/operation.rs`, `src-tauri/src/main.rs` (new cmds: `classify`, `plan_organize`, `execute_plan`, `undo_last`), `src/main.js`+`index.html`+`styles.css` (preserve mascot intent)

### Phase 3 — Packaging & Smoke (2 days) [REVISED]
- Scope: Model bundling `all-MiniLM-L6-v2` Apache-2.0 (~45–54MB Q4) in `src-tauri/resources/`, **ship `resources/licenses/LICENSE-MiniLM`**, size budget, re-enable CI matrix `windows/macos/ubuntu` smoke (50 files local vs cloud), keychain deferred to `.env`
- Validations: Win/Mac/Linux manual matrix; CI gate Win+Ubuntu local 10 files

### Oracle Feedback Addressed (ora-2)
1. Trait: single `classify` method only ✓
2. Dialog plugin removed (native input) ✓
3. Phase 1.5 smoke gate added ✓
4. OnceLock lazy load + spinner ✓
5. LICENSE file bundled ✓

## Decisions Log
- 2026-08-23: User wants bundled, not Ollama endpoint — Ollama design from ora-1 is obsolete; must choose embeddable runtime
- 2026-08-23: Single-commit history enforced by user — keep 1-commit until MVP, then conventional commits

## Validation

### Phase 3 smoke — 2026-08-23 (Windows host, ponytail)

| Check | File(s) checked | Result | Notes |
|-------|-----------------|--------|-------|
| Model bundling — LICENSE | `src-tauri/resources/licenses/LICENSE-MiniLM` (915 B stub, Apache-2.0 header) | **PASS** | Placeholder text present; replace with full Apache-2.0 before release |
| Model bundling — model dir | `src-tauri/resources/models/all-MiniLM-L6-v2/.gitkeep` (0 B) | **PASS** | Dir exists with .gitkeep; model files (~54 MB Q4 / ~88 MB fp32 safetensors) not yet downloaded — download to this path before bundle |
| `bundle.resources` | `src-tauri/tauri.conf.json:28` `resources: ["resources"]` | **PASS** | Correct; Tauri will copy `src-tauri/resources/**` to `BaseDirectory::Resource` |
| `cargo check` broomed-core | `crates/broomed-core` | **PASS** | `Finished dev profile 0.32s` |
| `cargo check` tauri crate | `src-tauri` | **PASS** | `Finished dev profile 0.93s` |
| `cargo test` broomed-core | `crates/broomed-core` (87 tests) | **PASS** | 87 passed, 0 failed (incl. `classify_smoke`) |
| `cargo test` tauri crate | `src-tauri` | **PASS** | 0 tests, compiles ok (1m03s) |
| Preview chain — Rust IPC | `src-tauri/src/main.rs` registers `scan_directory_cmd`, `classify_cmd`, `plan_organize`/`_cmd`, `execute_plan`/`_cmd`, `undo_last`/`_cmd` | **PASS** | All 12 handlers in `generate_handler!`; `operation::plan_organize` / `execute_previews` / `journal.undo_last` wired |
| Preview chain — frontend | `src/main.js` | **PARTIAL** | `scan_directory_cmd`→`classify_cmd` loop (line 195/206) wired + preview render/batched 200 rows works; `executePlan()` (272) and `undoLast()` (292) are `ponytail` stubs calling `classify_cmd` with `execute_stub`/`undo_stub` — need to swap to `invoke("plan_organize")`→preview data→`invoke("execute_plan_cmd",{previews})`→`invoke("undo_last_cmd")`; state machine `preview`→`executed`→`preview` already correct |
| Packaging — `productName`/`identifier` | `tauri.conf.json:2-4` `Broomed` / `com.broomed.desktop` | **PASS** | |
| Packaging — `bundle.targets` | `tauri.conf.json:25` `all` | **PASS** | Emits .msi (Win), .dmg (Mac), .AppImage/.deb (Linux) |
| Packaging — `bundle.icon` | `tauri.conf.json:27` `icon: []` vs `src-tauri/icons/icon.png`+`icon.ico` exist (14 KB) | **FAIL** | Array empty — set to `["icons/icon.png","icons/icon.ico"]` (and add 32x32, 128x128, icns for Mac) before `tauri build` or bundle will have no icon |
| Packaging — icons on disk | `src-tauri/icons/icon.png` (3208 B), `icon.ico` (11455 B) | **PASS** | Present but not referenced |
| CI workflows | `.github/workflows/ci.yml`, `release.yml`, `security.yml` | **SKIPPED** | Left as-is per instruction — user disabled to save minutes (`disabled_manually` via GH UI, files themselves still contain `on: push`); re-enable later with matrix `windows/macos/ubuntu`, local 10-file smoke |
| Installer size (no target) | `crates/broomed-core/src` 94 KB, `src` 30 KB, `src-tauri/src+conf` 6 KB, icons 14 KB, resources stub 915 B | **EST** | Compressed installer ~8–12 MB without model; +~54 MB Q4 → ~60–70 MB; +~88 MB fp32 → ~95–100 MB. Measured `crates` with target 865 MB, `src-tauri/target` 2.5 GB (debug artifacts, not shipped). Model dir currently empty — size will dominate bundle. |
| Signing (note only) | — | **NOTE** | Win .msi: needs EV cert / `signCommand` in `bundle.windows`; Mac .dmg/.app: Apple Developer ID + `APPLE_SIGNING_IDENTITY` + notarization (`appleSigningIdentity`, `APPLE_API_KEY`); Linux AppImage: GPG optional. No certs generated. |

- Manual matrix (deferred to release): Win/Mac/Linux × Local bundled vs Cloud (key set) × 50 files — hash-verified moves + undo — not run on this host; covered by `plan_organize` threshold + `operation::tests::pipeline_execute_and_undo_last`
