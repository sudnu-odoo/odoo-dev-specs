# One entry point for everything. `make check` must pass before a milestone is done.
.PHONY: install dev build test lint fmt check

install:            ## Install UI dependencies
	npm --prefix ui ci

dev:                ## Run the desktop app with hot reload
	cargo tauri dev

build:              ## Build .deb and AppImage bundles
	cargo tauri build

test:               ## Rust unit tests
	cargo test --workspace

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
