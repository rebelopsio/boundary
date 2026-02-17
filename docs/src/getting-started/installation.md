# Installation

## Pre-built Binaries

Download the latest release for your platform from [GitHub Releases](https://github.com/rebelopsio/boundary/releases).

Binaries are available for:

- **macOS** (Apple Silicon and Intel)
- **Linux** (x86_64)
- **Windows** (x86_64)

## Install from Source

With a Rust toolchain installed ([rustup.rs](https://rustup.rs)):

```bash
cargo install --git https://github.com/rebelopsio/boundary boundary
```

Or clone and build locally:

```bash
git clone https://github.com/rebelopsio/boundary.git
cd boundary
cargo build --release
# Binary is at target/release/boundary
```

## Verify Installation

```bash
boundary --version
```
