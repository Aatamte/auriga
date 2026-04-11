# CLI

## Overview

The `auriga` binary is the primary entry point for Auriga. It launches the TUI and provides self-update functionality. It is a thin wrapper — the actual TUI logic lives in the `auriga-app` binary, which `auriga` locates and launches.

## Commands

Running `auriga` with no arguments launches the TUI. Additional commands:

- **update** — checks GitHub Releases for a newer version and replaces the binary if one is found
- **version** — prints the current version
- **help** — shows available commands

## Install

Pre-built binaries are available via a shell script that detects the platform, downloads the correct release from GitHub, and installs to `~/.local/bin/`. The install location is configurable via an environment variable.

Building from source requires Rust 1.75+ and produces two binaries (`auriga` and `auriga-app`) that must be placed in the same directory.

## Self-Update

The update mechanism uses GitHub Releases. It compares the current version against the latest release tag, downloads the new binary if newer, and replaces itself in place. Both the CLI and the TUI binary are updated together.

## Configuration

On first run, the TUI creates a `.auriga/` directory in the current project root containing configuration (MCP port, disabled classifiers), grid layout, and the SQLite database.

## Release Process

Tagging a commit with a version tag triggers a GitHub Actions workflow that cross-compiles for macOS (ARM and Intel) and Linux (x86_64), strips the binaries, packages them as tarballs, and uploads them to a GitHub Release.
