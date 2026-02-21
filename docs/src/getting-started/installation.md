# Installation

## Homebrew (macOS / Linux)

```bash
brew install rebelopsio/tap/boundary
```

This installs both `boundary` and `boundary-lsp`.

## Pre-built Binaries

Download the latest release for your platform from [GitHub Releases](https://github.com/rebelopsio/boundary/releases).

Both `boundary` and `boundary-lsp` are included in each release archive. Binaries are available for:

- **macOS** (Apple Silicon and Intel)
- **Linux** (x86_64)
- **Windows** (x86_64)

## Install from Source

With a Rust toolchain installed ([rustup.rs](https://rustup.rs)):

```bash
cargo install --git https://github.com/rebelopsio/boundary boundary boundary-lsp
```

Or clone and build locally:

```bash
git clone https://github.com/rebelopsio/boundary.git
cd boundary
cargo build --release
# Binaries are at target/release/boundary and target/release/boundary-lsp
```

## Verify Installation

```bash
boundary --version
```
