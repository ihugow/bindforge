#!/usr/bin/env python3
"""Build the matched desktop + daemon RPM without installing or starting it."""
from pathlib import Path
import os
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    # The bundle mapping is deliberately rooted in this checkout, regardless of
    # a developer's global Cargo target directory.
    environment = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / "target"))
    subprocess.run(
        ["cargo", "build", "--release", "--locked", "-p", "remap-daemon", "--features", "kde-dbus"],
        cwd=ROOT, env=environment, check=True,
    )
    subprocess.run(
        ["npm", "run", "tauri", "--", "build", "--bundles", "rpm"],
        cwd=ROOT / "apps/desktop", env=environment, check=True,
    )
    for package in (ROOT / "target/release/bundle/rpm").glob("*.rpm"):
        print(package)


if __name__ == "__main__":
    main()
