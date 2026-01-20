#!/bin/bash
set -e

# Development Environment Setup Script
# This script helps set up the development environment for the Oatty CLI

echo "🦀 Oatty CLI (Rust) - Development Setup"
echo "========================================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print status messages
print_status() {
    echo -e "${BLUE}==>${NC} $1"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

# Check if we're in the project root
if [ ! -f "Cargo.toml" ] || [ ! -d "crates" ]; then
    print_error "This script must be run from the project root directory"
    exit 1
fi

print_status "Checking prerequisites..."

# Check for Rust
if ! command -v rustc &> /dev/null; then
    print_error "Rust is not installed. Please install from https://rustup.rs/"
    exit 1
else
    RUST_VERSION=$(rustc --version)
    print_success "Rust found: $RUST_VERSION"
fi

# Check for Cargo
if ! command -v cargo &> /dev/null; then
    print_error "Cargo is not found. Please reinstall Rust."
    exit 1
else
    CARGO_VERSION=$(cargo --version)
    print_success "Cargo found: $CARGO_VERSION"
fi

# Ensure the toolchain specified by rust-toolchain.toml is installed
print_status "Verifying Rust toolchain..."
TOOLCHAIN_CHANNEL=$(sed -n 's/^channel = \"\\(.*\\)\"$/\\1/p' rust-toolchain.toml | head -n 1)
if [ -z "$TOOLCHAIN_CHANNEL" ]; then
    print_warning "Could not detect toolchain channel from rust-toolchain.toml; continuing with current toolchain"
else
    if rustup toolchain list | grep -q "^${TOOLCHAIN_CHANNEL}"; then
        print_success "Toolchain installed: ${TOOLCHAIN_CHANNEL}"
    else
        print_warning "Installing toolchain: ${TOOLCHAIN_CHANNEL}..."
        rustup toolchain install "${TOOLCHAIN_CHANNEL}"
        print_success "Toolchain installed: ${TOOLCHAIN_CHANNEL}"
    fi
fi

# Check for required components
print_status "Checking required components..."
if rustup component list --installed | grep -q "clippy"; then
    print_success "Clippy is installed"
else
    print_warning "Installing clippy..."
    rustup component add clippy
fi

if rustup component list --installed | grep -q "rustfmt"; then
    print_success "Rustfmt is installed"
else
    print_warning "Installing rustfmt..."
    rustup component add rustfmt
fi

# Create .env file if it doesn't exist
if [ ! -f ".env" ]; then
    print_status "Creating .env file from template..."
    cp .env.example .env
    print_success "Created .env file - please edit it with your API key"
    print_warning "Don't forget to set OATTY_API_TOKEN in .env!"
else
    print_success ".env file already exists"
fi

# Create MCP config directory
MCP_CONFIG_DIR="$HOME/.config/oatty"
if [ ! -d "$MCP_CONFIG_DIR" ]; then
    print_status "Creating MCP config directory..."
    mkdir -p "$MCP_CONFIG_DIR"
    print_success "Created $MCP_CONFIG_DIR"
else
    print_success "MCP config directory exists"
fi

# Build the project
print_status "Building the project (this may take a few minutes)..."
if cargo build --workspace 2>&1 | tee /tmp/oatty-build.log; then
    print_success "Project built successfully"
else
    print_error "Build failed. Check /tmp/oatty-build.log for details"
    exit 1
fi

# Run tests
print_status "Running tests..."
if cargo test --workspace --quiet 2>&1 | tee /tmp/oatty-test.log; then
    print_success "All tests passed"
else
    print_warning "Some tests failed. Check /tmp/oatty-test.log for details"
fi

# Run clippy
print_status "Running clippy..."
if cargo clippy --workspace -- -D warnings 2>&1 | tee /tmp/oatty-clippy.log; then
    print_success "No clippy warnings"
else
    print_warning "Clippy found issues. Check /tmp/oatty-clippy.log for details"
fi

echo ""
echo "=========================================="
echo -e "${GREEN}Setup Complete!${NC}"
echo "=========================================="
echo ""
echo "Next steps:"
echo "  1. Edit .env with your Oatty API key"
echo "  2. Run the TUI: cargo run -p oatty-cli"
echo "  3. Or run CLI commands: cargo run -p oatty-cli -- apps list"
echo "  4. If no commands appear, import a registry catalog from the TUI Library view (e.g., schemas/samples/render-public-api.json)"
echo ""
echo "Development commands:"
echo "  - Build: cargo build --workspace"
echo "  - Test: cargo test --workspace"
echo "  - Lint: cargo clippy --workspace -- -D warnings"
echo "  - Format: cargo fmt --all"
echo ""
echo "VS Code/Cursor:"
echo "  - Open the project and install recommended extensions"
echo "  - Press F5 to start debugging with configured launch targets"
echo "  - Use Cmd+Shift+P → 'Tasks: Run Task' for quick commands"
echo ""
echo "Documentation:"
echo "  - DEVELOPMENT.md - Complete development guide"
echo "  - ARCHITECTURE.md - System architecture"
echo "  - README.md - User guide and features"
echo ""
print_success "Happy coding! 🦀"
