#!/usr/bin/env python3
"""Resolve (and optionally build) the real server workspace without client sources."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FORBIDDEN = ("kim-client", "kim-sdk", "kim-agent", "kim-desktop", "kim-codex", "codex-", "goose-", "flutter_rust_bridge", "rmcp")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--build", action="store_true")
    parser.add_argument("--target-dir", type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="kim-server-check-") as tmp:
        existing = Path(tmp) / "existing"
        existing.mkdir()
        sentinel = existing / "keep.txt"
        sentinel.write_text("caller-owned files must survive")
        refused = subprocess.run(
            [sys.executable, str(ROOT / "tools/build/prepare-server-context.py"), str(existing)],
            capture_output=True, text=True,
        )
        if refused.returncode == 0 or not sentinel.exists():
            raise RuntimeError("context generator replaced an existing destination")
        context = Path(tmp) / "context"
        subprocess.run([sys.executable, str(ROOT / "tools/build/prepare-server-context.py"), str(context)], check=True)
        manifest = context / "Cargo.toml"
        base = ["cargo", "--offline"] if args.offline else ["cargo"]
        metadata = subprocess.run(
            [*base, "metadata", "--manifest-path", str(manifest), "--locked", "--all-features", "--format-version", "1"],
            capture_output=True, text=True, check=True,
        )
        data = json.loads(metadata.stdout)
        bad = [p["name"] for p in data["packages"] if p["name"].startswith(FORBIDDEN)]
        if bad:
            raise RuntimeError(f"server dependency graph contains client/desktop packages: {bad}")
        for package in data["packages"]:
            if package["source"] is None and not Path(package["manifest_path"]).resolve().is_relative_to(context.resolve()):
                raise RuntimeError(f"local package escapes the isolated context: {package['manifest_path']}")
        for name in ("crates/client", "crates/agent", "third_party", "sdk", "app", "apps"):
            if (context / name).exists():
                raise RuntimeError(f"server context contains {name}")
        print(f"server graph: {len(data['workspace_members'])} local packages, {len(data['packages'])} resolved; no client/desktop/Codex")
        if args.build:
            command = [*base, "build", "--manifest-path", str(manifest), "--locked", "--all-features", "--message-format=json"]
            if args.target_dir:
                command += ["--target-dir", str(args.target_dir.resolve())]
            result = subprocess.run(command, capture_output=True, text=True)
            sys.stderr.write(result.stderr)
            if result.returncode:
                sys.stdout.write(result.stdout)
                return result.returncode
            names = {p["id"]: p["name"] for p in data["packages"]}
            artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
            built = {names[a["package_id"]] for a in artifacts if a.get("reason") == "compiler-artifact"}
            unexpected = [name for name in built if name.startswith(FORBIDDEN) or name in {"sqlx-sqlite", "libsqlite3-sys", "sqlx-mysql"}]
            if unexpected:
                raise RuntimeError(f"server build compiled forbidden packages: {unexpected}")
            expected = set(tomllib.loads(manifest.read_text())["workspace"]["metadata"]["kim-server"]["binaries"])
            binaries = {a["target"]["name"] for a in artifacts if a.get("reason") == "compiler-artifact" and a.get("executable")}
            if not expected <= binaries:
                raise RuntimeError(f"missing server binaries: {sorted(expected - binaries)}")
            print(f"server build: {len(built)} artifact packages; {', '.join(sorted(expected))}; no desktop or SQLite compilation")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as error:
        print(error.stderr or str(error), file=sys.stderr)
        raise SystemExit(error.returncode)
    except (OSError, ValueError, RuntimeError) as error:
        print(f"server context check: {error}", file=sys.stderr)
        raise SystemExit(1)
