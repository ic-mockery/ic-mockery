.PHONY: build build-canister test clean help

# Default target
all: build test

# Help message
help:
	@echo "Available targets:"
	@echo "  build         - Build all crates"
	@echo "  build-canister - Build only the canister"
	@echo "  test          - Run all tests"
	@echo "  clean         - Clean build artifacts"
	@echo "  help          - Show this help message"

# Build all crates
build:
	@echo "Building all crates..."
	cargo build --release
	@echo "Building canister..."
	cargo build --target wasm32-unknown-unknown --release --package canister

# Build only the canister
build-canister:
	@echo "Building canister..."
	cargo build --target wasm32-unknown-unknown --release --package canister

# Run tests
test: build-canister
	@echo "Running tests..."
	cargo test --workspace

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cargo clean
