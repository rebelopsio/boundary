# ==============================================================================
# Boundary - Static Analysis for Architectural Boundaries
# ==============================================================================

# ==============================================================================
# Rust Installation
#
#	You need Rust installed via rustup.
#	https://rustup.rs/
#
#	$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# ==============================================================================
# Install dependencies
#
#	Run these commands to install everything needed.
#	$ make dev-brew
#	$ make dev-setup

# ==============================================================================
# Define variables

VERSION         := $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
COMMIT          := $(shell git rev-parse --short HEAD)

# Detect OS for open command
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
	OPEN_CMD := open
else
	OPEN_CMD := xdg-open
endif

# ==============================================================================
# Install tooling and dependencies

dev-brew:
	brew update
	brew list pre-commit || brew install pre-commit

dev-setup: dev-brew
	rustup component add rustfmt clippy
	pre-commit install
	@echo "Development environment ready."

# ==============================================================================
# Building

build:
	cargo build

build-release:
	cargo build --release --bin boundary --bin boundary-lsp

install:
	cargo install --path crates/boundary --root $(HOME)/.cargo
	cargo install --path crates/boundary-lsp --root $(HOME)/.cargo

# ==============================================================================
# Development

run:
	cargo run --bin boundary -- $(ARGS)

analyze:
	cargo run --bin boundary -- analyze $(TARGET)

check:
	cargo run --bin boundary -- check $(TARGET)

# ==============================================================================
# Testing

test:
	cargo test --all

test-verbose:
	cargo test --all -- --nocapture

test-crate:
	@if [ -z "$(CRATE)" ]; then \
		echo "Error: CRATE required"; \
		echo "Example: make test-crate CRATE=boundary-core"; \
		exit 1; \
	fi
	cargo test -p $(CRATE)

# ==============================================================================
# Code quality

fmt:
	cargo fmt

fmt-check:
	cargo fmt -- --check

clippy:
	cargo clippy --all -- -D warnings

lint: fmt-check clippy

# ==============================================================================
# All CI checks (mirrors what CI runs)

ci: lint test build-release

# ==============================================================================
# Cleaning

clean:
	cargo clean

# ==============================================================================
# Version info

version:
	@echo "boundary $(VERSION) ($(COMMIT))"

# ==============================================================================
# Help

help:
	@echo "Usage: make <command>"
	@echo ""
	@echo "Setup:"
	@echo "  dev-brew          Install brew dependencies"
	@echo "  dev-setup         Full development environment setup"
	@echo ""
	@echo "Build:"
	@echo "  build             Debug build"
	@echo "  build-release     Optimized release build (boundary + boundary-lsp)"
	@echo "  install           Install both binaries to ~/.cargo/bin"
	@echo ""
	@echo "Development:"
	@echo "  run ARGS=...      Run boundary CLI with arguments"
	@echo "  analyze TARGET=.  Run boundary analyze on a target directory"
	@echo "  check TARGET=.    Run boundary check on a target directory"
	@echo ""
	@echo "Testing:"
	@echo "  test              Run all tests"
	@echo "  test-verbose      Run all tests with output"
	@echo "  test-crate CRATE= Run tests for a single crate"
	@echo ""
	@echo "Code Quality:"
	@echo "  fmt               Format code"
	@echo "  fmt-check         Check formatting"
	@echo "  clippy            Run clippy lints"
	@echo "  lint              Run all linters (fmt-check + clippy)"
	@echo "  ci                Run full CI suite locally"
	@echo ""
	@echo "Other:"
	@echo "  clean             Remove build artifacts"
	@echo "  version           Show version info"

.PHONY: build build-release install run analyze check test test-verbose test-crate \
	fmt fmt-check clippy lint ci clean version help dev-brew dev-setup
