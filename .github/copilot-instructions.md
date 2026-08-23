# GitHub Copilot Instructions

## Files and Directories to Ignore
Do NOT review or comment on:
- `target/` and build outputs
- `Cargo.lock` churn without dependency changes
- Generated Tauri schemas under `src-tauri/gen/`

## Focus Areas for Code Review
**DO focus on:**
- `crates/broomed-core/` — Rust core (filesystem safety, organization engine)
- `src-tauri/` — Tauri desktop shell
- `src/` — Frontend / mascot UI
- `.github/workflows/` — CI/CD

## Review Principles
- Rust correctness, safety, and idiomatic error handling
- No `unwrap()` on untrusted input at trust boundaries
- Test meaningful behavior, not implementation details
- Security (path traversal, symlink handling) is paramount

## Suppression Syntax
If a PR should suppress automated review noise, use:
```text
<!-- copilot: skip_review -->
```
