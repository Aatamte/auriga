#!/usr/bin/env python3
"""Build agent-orchestrator as native release binaries."""

import platform
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BINARIES = ["aorch", "orchestrator-app"]


def run(cmd: list[str], **kwargs) -> subprocess.CompletedProcess:
    print(f"→ {' '.join(cmd)}")
    return subprocess.run(cmd, check=True, cwd=ROOT, **kwargs)


def main():
    run(["cargo", "build", "--release"])

    system = platform.system()
    target_dir = ROOT / "target" / "release"

    for name in BINARIES:
        binary = target_dir / name
        if not binary.exists():
            print(f"ERROR: binary not found at {binary}", file=sys.stderr)
            sys.exit(1)

        if system in ("Darwin", "Linux"):
            run(["strip", str(binary)])

        size_mb = binary.stat().st_size / (1024 * 1024)

        dest = ROOT / name
        shutil.copy2(binary, dest)
        dest.chmod(0o755)

        print(f"  {name}: {size_mb:.1f} MB → {dest}")

    print(f"\nBuild complete — {system} {platform.machine()}")


if __name__ == "__main__":
    main()
