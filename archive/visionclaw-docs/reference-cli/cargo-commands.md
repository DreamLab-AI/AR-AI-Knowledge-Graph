---
title: Cargo Commands Reference
description: Rust cargo commands for VisionClaw development
category: reference
difficulty-level: intermediate
tags:
  - cli
  - cargo
  - rust
updated-date: 2025-01-29
---

# Cargo Commands Reference

Rust cargo commands for building, testing, and running VisionClaw.

---

## Build Commands

### Development Build

```bash
# Default development build
cargo build

# Build with all features
cargo build --all-features

# Build specific package
cargo build -p visionclaw-core
```

### Release Build

```bash
# Optimized release build
cargo build --release

# Release with specific target
cargo build --release --target x86_64-unknown-linux-gnu
```

### Feature Flags

| Feature | Description | Command |
|---------|-------------|---------|
| `gpu` | CUDA GPU acceleration | `cargo build --features gpu` |
| `simd` | SIMD optimizations | `cargo build --features simd` |
| `wasm` | WebAssembly target | `cargo build --target wasm32-unknown-unknown` |
| `xr` | XR/VR support | `cargo build --features xr` |

```bash
# Multiple features
cargo build --features "gpu,simd"

# All features
cargo build --all-features
```

---

## Run Commands

### Development Run

```bash
# Run with cargo
cargo run

# Run with arguments
cargo run -- --config config.yaml

# Run with environment variables
RUST_LOG=debug cargo run
```

### Release Run

```bash
# Run optimized binary
cargo run --release

# Run with specific features
cargo run --release --features gpu
```

---

## Test Commands

### Unit Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run tests in specific module
cargo test module_name::

# Run tests with output
cargo test -- --nocapture

# Run tests in release mode
cargo test --release
```

### Integration Tests

```bash
# Run integration tests only
cargo test --test '*'

# Run specific integration test
cargo test --test api_tests

# Run tests with filter
cargo test integration --test '*'
```

### Documentation Tests

```bash
# Run doc tests
cargo test --doc

# Run all tests including doc tests
cargo test --all
```

### Test Options

| Option | Description |
|--------|-------------|
| `--nocapture` | Show stdout/stderr |
| `--ignored` | Run ignored tests |
| `--test-threads=1` | Run tests serially |
| `-- --exact` | Match test name exactly |

---

## Code Quality Commands

### Formatting

```bash
# Check formatting
cargo fmt --check

# Apply formatting
cargo fmt

# Format specific file
cargo fmt -- src/main.rs
```

### Linting

```bash
# Run clippy
cargo clippy

# Fix clippy warnings
cargo clippy --fix

# Clippy with all features
cargo clippy --all-features

# Deny warnings
cargo clippy -- -D warnings
```

### Documentation

```bash
# Build documentation
cargo doc

# Build and open docs
cargo doc --open

# Build docs with private items
cargo doc --document-private-items
```

---

## Benchmarking

```bash
# Run benchmarks
cargo bench

# Run specific benchmark
cargo bench bench_name

# Run benchmarks with features
cargo bench --features gpu
```

---

## Dependency Management

### Check Dependencies

```bash
# Check outdated dependencies
cargo outdated

# Audit for security vulnerabilities
cargo audit

# Tree of dependencies
cargo tree
```

### Update Dependencies

```bash
# Update all dependencies
cargo update

# Update specific dependency
cargo update -p serde
```

---

## Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `RUST_LOG` | Logging level | `debug`, `info`, `warn`, `error` |
| `RUST_BACKTRACE` | Enable backtraces | `1` or `full` |
| `CARGO_INCREMENTAL` | Incremental compilation | `1` or `0` |
| `RUSTFLAGS` | Compiler flags | `-C target-cpu=native` |

```bash
# Example with multiple env vars
RUST_LOG=visionclaw=debug RUST_BACKTRACE=1 cargo run
```

---

## Related Documentation

- [Docker Commands](./docker-commands.md)
- [CLI Reference](./README.md)
- [Deployment Guide](../../how-to/deployment-guide.md)
