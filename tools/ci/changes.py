#!/usr/bin/env python3
"""Select CI jobs from the complete Git diff, without GitHub path-list limits."""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

JOBS = ("rust", "server", "postgres", "web", "web_e2e", "mobile", "media")
RUST_CONFIG = {
    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "rustfmt.toml",
    "clippy.toml", ".gitmodules",
}
NPM_CONFIG = {"package.json", "package-lock.json", ".npmrc"}


def select_jobs(paths: list[str] | None) -> dict[str, bool]:
    # A manual run or an unavailable diff must run the whole suite.
    if paths is None:
        return dict.fromkeys(JOBS, True)
    selected = set()
    for path in paths:
        if path.startswith((".github/", "tools/ci/")):
            return dict.fromkeys(JOBS, True)
        if path in RUST_CONFIG or path.startswith(".cargo/"):
            selected.update(("rust", "server", "postgres", "web_e2e", "mobile"))
        elif path in NPM_CONFIG:
            selected.update(("web", "web_e2e", "media"))
        elif path.startswith(("crates/shared/", "crates/server/", "services/",
                              "examples/kimbench/", "examples/pkt-client/",
                              "tools/kim-server-migrate/", "tools/build/")):
            selected.update(("rust", "server", "postgres", "web_e2e"))
            if path.startswith("crates/shared/") or path.endswith("/Cargo.toml"):
                selected.add("mobile")
        elif path == "goose" or path.startswith(("crates/client/", "crates/agent/", "third_party/", "goose/")):
            selected.update(("rust", "mobile"))
        elif path.startswith("examples/"):
            selected.add("rust")
        elif path in {"Dockerfile", ".dockerignore"}:
            selected.add("server")
        elif path.startswith("deploy/"):
            selected.add("server")
            if path.endswith(".sql"):
                selected.add("postgres")
        elif path.startswith("sdk/web/"):
            selected.add("web")
            if path.startswith(("sdk/web/src/", "sdk/web/tests/")) or Path(path).name.startswith(("package", "vitest")):
                selected.add("web_e2e")
        elif path.startswith("sdk/media/"):
            selected.add("media")
        elif path.startswith("sdk/mobile/"):
            selected.add("mobile")
        elif path.startswith(("docs/", "books/", "research/", ".agents/", ".idea/", ".vscode/", ".codegraph/")) or path in {
            "README.md", "LICENSE", "AGENTS.md", "skills-lock.json", ".gitignore",
        }:
            continue
        else:
            # New components and build/gate scripts need coverage until classified.
            return dict.fromkeys(JOBS, True)
    return {job: job in selected for job in JOBS}


def changed_paths(event_name: str, event: dict, head: str, root: Path) -> list[str] | None:
    if event_name == "pull_request":
        pr = event["pull_request"]
        diff_range = f"{pr['base']['sha']}...{pr['head']['sha']}"
    elif event_name == "push" and event.get("before", "").strip("0"):
        diff_range = f"{event['before']}..{head}"
    else:
        return None
    try:
        diff = subprocess.run(
            ["git", "diff", "--no-renames", "--name-only", "-z", diff_range, "--"],
            cwd=root, capture_output=True, check=True,
        )
    except subprocess.CalledProcessError as error:
        print(f"::warning::Cannot determine changed files; running all jobs: {error.stderr.decode(errors='replace').strip()}")
        return None
    return [os.fsdecode(path) for path in diff.stdout.split(b"\0") if path]


def main() -> None:
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
    paths = changed_paths(os.environ["GITHUB_EVENT_NAME"], event, os.environ["GITHUB_SHA"], Path.cwd())
    selected = select_jobs(paths)
    outputs = "".join(f"{job}={str(enabled).lower()}\n" for job, enabled in selected.items())
    with Path(os.environ["GITHUB_OUTPUT"]).open("a") as output:
        output.write(outputs)
    print(outputs, end="")
    with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a") as summary:
        summary.write("## CI scope\n\n| Job | Selected |\n|---|---|\n")
        for job, enabled in selected.items():
            summary.write(f"| {job} | {'yes' if enabled else 'no'} |\n")
        summary.write(f"\nChanged paths: {len(paths) if paths is not None else 'full run (manual or unavailable diff)'}.\n")


if __name__ == "__main__":
    main()
