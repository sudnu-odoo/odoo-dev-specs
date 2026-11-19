# One entry point for everything. `make check` must pass before a milestone is done.
.PHONY: install dev build test test-live lint fmt check

install:            ## Install UI dependencies
	npm --prefix ui ci

dev:                ## Run the desktop app with hot reload
	cargo tauri dev

build:              ## Build the .deb (with the screenshot tool inside)
	cargo tauri build --bundles deb

test:               ## Rust and UI tests (unit + integration)
	cargo test --workspace
	npm --prefix ui run -s test

test-live:          ## End-to-end with the real, logged-in Claude Code (costs tokens)
	cargo build -p specs-mcp
	cargo test -p specs-core --test live_claude -- --ignored
	cargo test -p specs-core --test live_antigravity -- --ignored
	cargo test -p odoo-dev-specs --test live_providers -- --ignored --nocapture
	cargo test -p odoo-runtime --test live_runtime -- --ignored

lint:               ## Static checks (no changes)
	cargo clippy --workspace --all-targets -- -D warnings
	npm --prefix ui run -s lint
	npm --prefix ui run -s typecheck

fmt:                ## Auto-format Rust and UI code
	cargo fmt --all
	npm --prefix ui run -s format

check:              ## Everything CI would run
	cargo fmt --all --check
	npm --prefix ui run -s format:check
	$(MAKE) lint
	$(MAKE) test
