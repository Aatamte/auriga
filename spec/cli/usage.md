# CLI — Technical Specification

## Binary

Name: `aorch` (crate: `orchestrator-cli`)

Locates `orchestrator-app` relative to its own binary path:
```rust
std::env::current_exe()?.parent().join("orchestrator-app")
```

## Commands

| Input | Action |
|---|---|
| `aorch` | Launch `orchestrator-app` via `Command::new(bin).status()` |
| `aorch update` | Self-update via GitHub Releases |
| `aorch version` | Print `env!("CARGO_PKG_VERSION")` |
| `aorch help` | Print usage |

## Self-Update

Uses `self_update` crate:
```rust
self_update::backends::github::Update::configure()
    .repo_owner("Aatamte")
    .repo_name("agent-orchestrator")
    .bin_name("aorch")
    .current_version(VERSION)
    .show_download_progress(true)
    .build()?
    .update()?
```

Checks latest GitHub Release tag against `CARGO_PKG_VERSION`. Downloads and replaces if newer.

## Install Script

`scripts/install.sh` — platform detection and binary download:

| `uname -s` / `uname -m` | Target |
|---|---|
| Darwin / arm64 | `aarch64-apple-darwin` |
| Darwin / x86_64 | `x86_64-apple-darwin` |
| Linux / x86_64 | `x86_64-unknown-linux-gnu` |

Downloads `aorch-<target>.tar.gz` from GitHub Releases, extracts `aorch` + `orchestrator-app`, installs to `$INSTALL_DIR` (default `~/.local/bin`).

Environment variables:
- `INSTALL_DIR` — override install location
- `VERSION` — install specific version (default: latest)

## Release Artifacts

Each GitHub Release contains:
```
aorch-aarch64-apple-darwin.tar.gz
aorch-x86_64-apple-darwin.tar.gz
aorch-x86_64-unknown-linux-gnu.tar.gz
```

Each tarball contains two binaries: `aorch` and `orchestrator-app`, both stripped.

## Build Script

`scripts/build.py`:
1. `cargo build --release`
2. Strip binaries (`strip` on macOS/Linux)
3. Copy `aorch` and `orchestrator-app` to project root
4. Print sizes
