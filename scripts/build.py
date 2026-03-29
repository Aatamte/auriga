#!/usr/bin/env python3
"""Build claude-manager as a native release binary."""

import platform
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BINARY_NAME = "claude-manager"


def run(cmd: list[str], **kwargs) -> subprocess.CompletedProcess:
    print(f"→ {' '.join(cmd)}")
    return subprocess.run(cmd, check=True, cwd=ROOT, **kwargs)


def main():
    # Build release binary
    run(["cargo", "build", "--release"])

    # Locate the compiled binary
    target_bin = ROOT / "target" / "release" / BINARY_NAME
    if not target_bin.exists():
        print(f"ERROR: binary not found at {target_bin}", file=sys.stderr)
        sys.exit(1)

    # Strip the binary for smaller size
    system = platform.system()
    if system in ("Darwin", "Linux"):
        run(["strip", str(target_bin)])

    size_mb = target_bin.stat().st_size / (1024 * 1024)
    print(f"\nBuild complete: {target_bin}")
    print(f"Size: {size_mb:.1f} MB")
    print(f"Platform: {system} {platform.machine()}")

    # Copy to project root for convenience
    dest = ROOT / BINARY_NAME
    shutil.copy2(target_bin, dest)
    dest.chmod(0o755)
    print(f"Copied to: {dest}")


if __name__ == "__main__":
    main()
