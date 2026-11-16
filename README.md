# Odoo Dev Specs

Desktop app for the Odoo business team: describe a feature in chat, and an AI assistant builds an
Excalidraw dev spec with real screenshots from a **fresh local Odoo database** (never customer data).

## Requirements (Ubuntu)

- [odev](https://github.com/odoo-odev/odev) with Odoo community + enterprise source checked out
- Rust (stable) and Node.js 20+
- Tauri system libraries:
  ```bash
  sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
    librsvg2-dev libayatana-appindicator3-dev
  cargo install tauri-cli --version "^2" --locked
  ```
- At least one AI CLI, logged in: Claude Code (`claude`) or Gemini CLI (`gemini`)
- Google Chrome or Chromium (for screenshots)

## Development

```bash
make install   # UI dependencies
make dev       # run the app with hot reload
make check     # format check, lint, type check, tests — must pass before merging
make build     # .deb + AppImage in target/release/bundle/
```

## Layout

| Path | What |
|---|---|
| `src-tauri/` | Desktop app core (Rust): window, commands, startup tasks |
| `crates/odev-core/` | odev integration shared by the app and the MCP server |
| `ui/` | React + TypeScript UI (sidebar, chat, Excalidraw panel) |

On every start the app moves `[pruning] date` in `~/.config/odev/odev.cfg` 5 years ahead so odev
never stops to ask about old databases. A `.bak` copy is written before any change.
