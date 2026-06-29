---
title: CLI Reference
description: Command-line interface reference for VisionClaw development and deployment
category: reference
difficulty-level: intermediate
tags:
  - cli
  - cargo
  - docker
updated-date: 2025-01-29
---

# CLI Reference

Command-line interface reference for VisionClaw development and deployment.

---

## Documentation Index

| Topic | File | Description |
|-------|------|-------------|
| **Cargo Commands** | [cargo-commands.md](./cargo-commands.md) | Rust build, test, run options |
| **Docker Commands** | [docker-commands.md](./docker-commands.md) | Docker and docker-compose commands |

---

## Quick Reference

### Build & Run

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run with default features
cargo run

# Run with specific features
cargo run --features gpu

# Run tests
cargo test
```

### Docker

```bash
# Start services
docker-compose up -d

# View logs
docker-compose logs -f visionclaw

# Stop services
docker-compose down

# Rebuild container
docker-compose build --no-cache
```

---

## Global CLI Options

| Option | Short | Description |
|--------|-------|-------------|
| `--config` | `-c` | Configuration file path |
| `--verbose` | `-v` | Increase verbosity |
| `--quiet` | `-q` | Suppress output |
| `--help` | `-h` | Show help |

---

## Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `RUST_LOG` | Logging level | `debug`, `info`, `warn`, `error` |
| `RUST_BACKTRACE` | Enable backtraces | `1` or `full` |

---

## Related Documentation

- [Deployment Guide](../../how-to/deployment-guide.md)
- [Configuration Reference](../configuration/README.md)
