# Conventions for Odoo Dev Specs

Read README.md for setup. Architecture: Tauri 2 (Rust core) + React/TS UI + Excalidraw; AI providers
are existing CLIs (Claude Code, Gemini CLI) run headless; diagram edits go through our own MCP server.

## Rules
- `make check` must pass before any change is considered done (fmt, clippy `-D warnings`, eslint,
  prettier, tsc, cargo tests).
- Business logic lives in Rust library crates under `crates/` with unit tests; `src-tauri/` only wires
  commands and state. React components stay presentational; Tauri calls go through `ui/src/lib/`.
- One concern per module/file. No dead code, no commented-out code, no placeholder data left in place
  after the feature that replaces it lands.
- Errors: `anyhow` with `.context(...)` in crates; Tauri commands return `Result<T, String>` with a
  message a business user can understand.
- File writes to user config are atomic (write temp + rename) and keep a `.bak`.
- odev is always called non-interactively (`-f`, explicit `-V` on create, `--log-level=warn`).
- The AI never gets shell access or write access outside the session folder.
- Pin exact versions for new npm dependencies; prefer well-known crates.
