#!/usr/bin/env python3
"""Confirm the root npm workspace links the local SDK and Media packages."""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "package-lock.json"


def main() -> int:
    lock = json.loads(LOCK.read_text())
    packages = lock.get("packages", {})
    expected = {
        "node_modules/@kim/web-sdk": "sdk/web",
        "node_modules/@kim/media": "sdk/media",
    }
    failures = []
    for name, target in expected.items():
        item = packages.get(name)
        if not item or not item.get("link") or item.get("resolved") != target:
            failures.append(f"{name} is not linked to {target}")
        pkg = json.loads((ROOT / target / "package.json").read_text())
        if pkg.get("version") != "0.1.0":
            failures.append(f"{target} version drifted from 0.1.0")
    if failures:
        print("npm workspace check failed:", file=sys.stderr)
        for item in failures:
            print(f"- {item}", file=sys.stderr)
        return 1
    print("npm workspace check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
