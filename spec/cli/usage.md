# CLI — Technical Specification

## Binary

Name: `auriga` (crate: `auriga-cli`)

Locates `auriga-app` relative to its own binary path:
```rust
std::env::current_exe()?.parent().join("auriga-app")
```

## Commands

| Input | Action |
|---|---|
| `auriga` | Launch `auriga-app` via `Command::new(bin).status()` |
| `auriga update` | Self-update via GitHub Releases |
| `auriga version` | Print `env!("CARGO_PKG_VERSION")` |
| `auriga help` | Print usage |

## Self-Update

Uses `self_update` crate:
```rust
self_update::backends::github::Update::configure()
    .repo_owner("Aatamte")
    .repo_name("auriga")
    .bin_name("auriga")
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

Downloads `auriga-<target>.tar.gz` from GitHub Releases, extracts `auriga` + `auriga-app`, installs to `$INSTALL_DIR` (default `~/.local/bin`).

Environment variables:
- `INSTALL_DIR` — override install location
- `VERSION` — install specific version (default: latest)

## Release Artifacts

Each GitHub Release contains:
```
auriga-aarch64-apple-darwin.tar.gz
auriga-x86_64-apple-darwin.tar.gz
auriga-x86_64-unknown-linux-gnu.tar.gz
```

Each tarball contains two binaries: `auriga` and `auriga-app`, both stripped.

## Build Script

`scripts/build.py`:
1. `cargo build --release`
2. Strip binaries (`strip` on macOS/Linux)
3. Copy `auriga` and `auriga-app` to project root
4. Print sizes
