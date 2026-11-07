#!/usr/bin/env python3
"""Fail when a crate's normal/build dependency closure crosses a role boundary.

Dev-dependencies are ignored except for the schema rule, which includes them.
Named exceptions are current violations with the slice that must remove them.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# (from_package, to_package_or_prefix, reason) still present on this branch.
EXCEPTIONS = {
    ("royal", "chat"): "R05 removes royal's normal dependency on chat",
}

# package -> forbidden package names or prefixes in the normal/build closure.
FORBIDDEN = {
    "kim-agent-model": ("kim-agent-host", "kim-agent-codec", "goose-", "rmcp", "codex-"),
    "kim-agent-codec": ("kim-agent-host", "goose-", "rmcp", "codex-", "kim-desktop-runtime"),
    "kim-sdk": ("kim-agent-host", "kim-desktop-runtime", "kim-codex-helper", "goose-", "rmcp"),
    "kim-agent-host": ("kim-sdk", "kim-desktop-runtime"),
    "kim-gateway-runtime": ("kim-ws",),
    "tgateway": ("gateway",),
    "gateway": ("kim-tcp",),
}

SCHEMA_PACKAGE = "kim-server-schema"
SCHEMA_ALLOWED = {"kim-server-schema", "kim-server-migrate"}


def metadata() -> dict:
    proc = subprocess.run(
        ["cargo", "metadata", "--format-version", "1"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(proc.stdout)


def dep_names(node: dict, kind: str) -> list[str]:
    names = []
    for dep in node.get("deps", []):
        kinds = dep.get("dep_kinds") or []
        if any(item.get("kind") == kind for item in kinds):
            names.append(dep["name"])
    return names


def closure(nodes: dict[str, dict], start: str, kinds: set[str]) -> set[str]:
    seen = set()
    stack = [start]
    while stack:
        name = stack.pop()
        if name in seen or name not in nodes:
            continue
        seen.add(name)
        for kind in kinds:
            for dep in dep_names(nodes[name], kind):
                if dep not in seen:
                    stack.append(dep)
    seen.discard(start)
    return seen


def forbidden_hit(package: str, dep: str) -> bool:
    for banned in FORBIDDEN.get(package, ()):
        if banned.endswith("-"):
            if dep.startswith(banned):
                return True
        elif dep == banned:
            return True
    return False


def main() -> int:
    data = metadata()
    nodes = {}
    for node in data["resolve"]["nodes"]:
        name = node["id"].split(" ")[0]
        nodes[name] = node

    failures = []
    for package, banned in FORBIDDEN.items():
        if package not in nodes:
            continue
        reached = closure(nodes, package, {"normal", "build"})
        for dep in sorted(reached):
            if not forbidden_hit(package, dep):
                continue
            if (package, dep) in EXCEPTIONS:
                continue
            failures.append(
                f"{package} normal/build reaches {dep}; forbidden by {banned}"
            )

    if SCHEMA_PACKAGE in nodes:
        for package, node in nodes.items():
            if package in SCHEMA_ALLOWED:
                continue
            reached = closure(nodes, package, {"normal", "build", "dev"})
            if SCHEMA_PACKAGE in reached:
                failures.append(
                    f"{package} links {SCHEMA_PACKAGE} in normal/build/dev; only kim-server-migrate may"
                )

    for (src, dst), reason in sorted(EXCEPTIONS.items()):
        if src not in nodes:
            continue
        reached = closure(nodes, src, {"normal", "build"})
        if dst not in reached:
            failures.append(
                f"exception {src} -> {dst} is stale ({reason}); delete it"
            )

    if failures:
        print("crate boundary check failed:", file=sys.stderr)
        for item in failures:
            print(f"- {item}", file=sys.stderr)
        return 1
    print("crate boundary check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
