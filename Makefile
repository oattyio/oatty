.PHONY: help build build-release clean test test-watch check fmt fmt-check clippy run run-tui run-cli doc setup pre-commit install-tools

# Default target
.DEFAULT_GOAL := help

# Colors for output
BLUE := \033[0;34m
GREEN := \033[0;32m
YELLOW := \033[1;33m
NC := \033[0m # No Color

SERVICE ?= core-api

help: ## Show this help message
	@echo "$(BLUE)Oatty CLI (Rust) - Development Commands$(NC)"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(GREEN)%-20s$(NC) %s\n", $$1, $$2}'
	@echo ""
	@echo "$(YELLOW)Environment Variables:$(NC)"
	@echo "  OATTY_LOG       - Log level: error|warn|info|debug|trace (default: info)"
	@echo "  TUI_THEME        - Theme: dracula|dracula_hc|nord|nord_hc (default: dracula)"

setup: ## Run initial setup script
	@echo "$(BLUE)==> Running development setup...$(NC)"
	@bash scripts/dev-setup.sh

build: ## Build all workspace crates (debug)
	@echo "$(BLUE)==> Building workspace (debug)...$(NC)"
	@cargo build --workspace

build-release: ## Build all workspace crates (optimized)
	@echo "$(BLUE)==> Building workspace (release)...$(NC)"
	@cargo build --workspace --release
	@echo "$(GREEN)✓ Release binary: target/release/oatty$(NC)"

clean: ## Clean build artifacts
	@echo "$(BLUE)==> Cleaning build artifacts...$(NC)"
	@cargo clean

test: ## Run all tests
	@echo "$(BLUE)==> Running tests...$(NC)"
	@cargo test --workspace

test-watch: ## Run tests in watch mode (requires cargo-watch)
	@echo "$(BLUE)==> Running tests in watch mode...$(NC)"
	@cargo watch -x "test --workspace"

check: ## Fast compilation check
	@echo "$(BLUE)==> Checking compilation...$(NC)"
	@cargo check --workspace

fmt: ## Format all code
	@echo "$(BLUE)==> Formatting code...$(NC)"
	@cargo fmt --all

fmt-check: ## Check code formatting
	@echo "$(BLUE)==> Checking code format...$(NC)"
	@cargo fmt --all -- --check

clippy: ## Run clippy lints
	@echo "$(BLUE)==> Running clippy...$(NC)"
	@cargo clippy --workspace -- -D warnings

clippy-fix: ## Run clippy with auto-fix
	@echo "$(BLUE)==> Running clippy with auto-fix...$(NC)"
	@cargo clippy --workspace --fix

run-tui: ## Run the TUI (interactive mode)
	@echo "$(BLUE)==> Launching TUI...$(NC)"
	@cargo run -p oatty-cli

run-cli: ## Run CLI with arguments (use ARGS="apps list")
	@echo "$(BLUE)==> Running CLI: $(ARGS)$(NC)"
	@cargo run -p oatty-cli -- $(ARGS)

run-apps-list: ## Run: apps list
	@echo "$(BLUE)==> Running: apps list$(NC)"
	@cargo run -p oatty-cli -- apps list

run-apps-info: ## Run: apps info (use APP=my-app)
	@echo "$(BLUE)==> Running: apps info $(APP)$(NC)"
	@cargo run -p oatty-cli -- apps info $(APP)

doc: ## Generate and open documentation
	@echo "$(BLUE)==> Generating documentation...$(NC)"
	@cargo doc --workspace --no-deps --open

doc-all: ## Generate documentation including dependencies
	@echo "$(BLUE)==> Generating documentation (with deps)...$(NC)"
	@cargo doc --workspace --open

pre-commit: fmt clippy test ## Run all pre-commit checks
	@echo "$(GREEN)✓ All pre-commit checks passed!$(NC)"

install-tools: ## Install useful development tools
	@echo "$(BLUE)==> Installing development tools...$(NC)"
	@cargo install cargo-watch || true
	@cargo install cargo-edit || true
	@cargo install cargo-nextest || true
	@echo "$(GREEN)✓ Development tools installed$(NC)"

bench: ## Run benchmarks (if any)
	@echo "$(BLUE)==> Running benchmarks...$(NC)"
	@cargo bench --workspace

outdated: ## Check for outdated dependencies
	@echo "$(BLUE)==> Checking for outdated dependencies...$(NC)"
	@cargo outdated

update: ## Update dependencies
	@echo "$(BLUE)==> Updating dependencies...$(NC)"
	@cargo update

tree: ## Show dependency tree
	@echo "$(BLUE)==> Showing dependency tree...$(NC)"
	@cargo tree

bloat: ## Analyze binary size
	@echo "$(BLUE)==> Analyzing binary size...$(NC)"
	@cargo bloat --release -p oatty-cli

# Quick run examples
.PHONY: example-apps-list example-apps-create example-tui
example-apps-list: ## Example: List all apps
	@$(MAKE) run-apps-list

example-apps-create: ## Example: Create an app (use NAME=my-app)
	@echo "$(BLUE)==> Running: apps create --name $(NAME)$(NC)"
	@cargo run -p oatty-cli -- apps create --name $(NAME)

example-tui: ## Example: Launch TUI with debug logging
	@OATTY_LOG=debug TUI_THEME=dracula $(MAKE) run-tui
