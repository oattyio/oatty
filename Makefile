.PHONY: help build build-release clean test check fmt fmt-check clippy clippy-fix run run-tui run-cli doc doc-all \
	setup licenses-prepare licenses-verify npm-package npm-package-umbrella npm-publish release-package release-metadata \
	pre-commit install-tools bench outdated update tree bloat test-watch

# Default target
.DEFAULT_GOAL := help

# Core build and test operations delegate straight to Cargo. Auxiliary,
# project-specific development utilities (setup, licensing materials, packaging,
# npm publication, release metadata) delegate to `cargo xtask` (crates/xtask).

help: ## Show this help message
	@echo "Oatty CLI (Rust) - Development Commands"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-24s %s\n", $$1, $$2}'
	@echo ""
	@echo "Environment Variables:"
	@echo "  OATTY_LOG       - Log level: error|warn|info|debug|trace (default: info)"
	@echo "  TUI_THEME       - Theme: dracula|dracula_hc|nord|nord_hc (default: dracula)"

build: ## Build all workspace crates (debug)
	@cargo build --workspace

build-release: ## Build all workspace crates (optimized)
	@cargo build --workspace --release
	@echo "Release binary: target/release/oatty"

clean: ## Clean build artifacts
	@cargo clean

test: ## Run all tests
	@cargo test --workspace

test-watch: ## Run tests in watch mode (requires cargo-watch)
	@cargo watch -x "test --workspace"

check: ## Fast compilation check
	@cargo check --workspace

fmt: ## Format all code
	@cargo fmt --all

fmt-check: ## Check code formatting
	@cargo fmt --all -- --check

clippy: ## Run clippy lints
	@cargo clippy --workspace -- -D warnings

clippy-fix: ## Run clippy with auto-fix
	@cargo clippy --workspace --fix

doc: ## Generate and open documentation
	@cargo doc --workspace --no-deps --open

doc-all: ## Generate documentation (with dependencies)
	@cargo doc --workspace --open

run-tui: ## Run the TUI (interactive mode)
	@cargo run -p oatty-cli

run-cli: ## Run the CLI with arguments (make run-cli ARGS="apps list")
	@echo "Running CLI: $(ARGS)"
	@cargo run -p oatty-cli -- $(ARGS)

# Auxiliary: workspace development tasks (crates/xtask)
setup: ## Set up the development environment (toolchain, config, first checks)
	@cargo xtask setup

licenses-prepare: ## Prepare third-party licensing materials and the staleness manifest
	@cargo xtask licenses prepare

licenses-verify: ## Verify third-party licensing materials are current and complete
	@cargo xtask licenses verify

npm-package: ## Pack one native npm payload (make npm-package BINARY=... TARGET=...)
	@cargo xtask npm package --binary $(BINARY) --target $(TARGET) --output dist

npm-package-umbrella: ## Pack the launcher-only npm umbrella package
	@cargo xtask npm package --umbrella --output dist

npm-publish: ## Publish validated npm release artifacts (make npm-publish DIRECTORY=dist)
	@cargo xtask npm publish --directory $(DIRECTORY)

release-package: ## Package a signed release archive (make release-package BINARY=... TARGET=... VERSION=...)
	@cargo xtask release-package --binary $(BINARY) --target $(TARGET) --version $(VERSION)

release-metadata: ## Validate Cargo, npm, and release tag versions for a release
	@cargo xtask release-metadata

pre-commit: fmt clippy test ## Run all pre-commit checks
	@echo "All pre-commit checks passed!"

install-tools: ## Install development tools (cargo-watch, cargo-edit, cargo-nextest, cargo-about)
	@cargo install cargo-watch || true
	@cargo install cargo-edit || true
	@cargo install cargo-nextest || true
	@cargo install cargo-about --version 0.9.2 --features cli --locked || true
	@echo "Development tools installed"

bench: ## Run benchmarks (if any)
	@cargo bench --workspace

outdated: ## Check for outdated dependencies
	@cargo outdated

update: ## Update dependencies
	@cargo update

tree: ## Show dependency tree
	@cargo tree

bloat: ## Analyze binary size
	@cargo bloat --release -p oatty-cli

example-tui: ## Launch the TUI with debug logging
	@OATTY_LOG=debug TUI_THEME=dracula cargo run -p oatty-cli
