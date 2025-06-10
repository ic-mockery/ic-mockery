# Makefile

# Toggle features here
FEATURES := ic-mockery

# Cargo commands with feature flag
CARGO := cargo build --target wasm32-unknown-unknown --release --locked
CARGO_FLAGS := --features $(FEATURES)

.PHONY: all build run test clean

all: build

build:
	$(CARGO) $(CARGO_FLAGS)