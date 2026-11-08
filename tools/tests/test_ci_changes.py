"""Regression coverage for CI routing: omissions silently remove test coverage."""

import importlib.util
import subprocess
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("ci_changes", Path(__file__).resolve().parents[1] / "ci/changes.py")
changes = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(changes)


class JobSelectionTests(unittest.TestCase):
    def selected(self, *paths):
        return {job for job, enabled in changes.select_jobs(list(paths)).items() if enabled}

    def test_docs_need_no_compilers(self):
        self.assertEqual(self.selected("README.md", "docs/deploy.md", ".agents/skills/a.md"), set())

    def test_server_does_not_build_mobile(self):
        self.assertEqual(self.selected("services/royal/src/bots.rs"), {"rust", "server", "postgres", "web_e2e"})

    def test_shared_code_covers_both_clients(self):
        self.assertEqual(self.selected("crates/shared/kim-protocol/proto/chat.proto"),
                         {"rust", "server", "postgres", "web_e2e", "mobile"})

    def test_desktop_and_ffi_changes_run_mobile(self):
        for path in ("crates/agent/kim-codex-helper/src/main.rs", "crates/client/kim-client/src/lib.rs", "third_party/codex", "goose"):
            with self.subTest(path=path):
                self.assertEqual(self.selected(path), {"rust", "mobile"})

    def test_workspace_and_server_manifests_cover_native_builds(self):
        for path in ("Cargo.lock", "Cargo.toml", ".cargo/config.toml", "services/chat/Cargo.toml"):
            with self.subTest(path=path):
                self.assertIn("mobile", self.selected(path))
                self.assertIn("postgres", self.selected(path))

    def test_mobile_and_media_are_independent(self):
        self.assertEqual(self.selected("sdk/mobile/lib/main.dart"), {"mobile"})
        self.assertEqual(self.selected("sdk/media/src/index.ts"), {"media"})

    def test_web_app_does_not_build_rust(self):
        self.assertEqual(self.selected("sdk/web/app/main.tsx"), {"web"})
        self.assertEqual(self.selected("sdk/web/src/client.ts"), {"web", "web_e2e"})
        self.assertEqual(self.selected("sdk/web/vitest.config.ts"), {"web", "web_e2e"})

    def test_workspace_npm_lock_covers_all_npm_consumers(self):
        self.assertEqual(self.selected("package-lock.json"), {"web", "web_e2e", "media"})

    def test_migrations_and_image_inputs(self):
        self.assertEqual(self.selected("deploy/backfill-inbox.sql"), {"server", "postgres"})
        self.assertEqual(self.selected("Dockerfile"), {"server"})

    def test_unknown_paths_and_ci_changes_run_everything(self):
        for path in ("new-component/a.rs", ".github/workflows/ci.yml", "tools/ci/changes.py", "tools/check_crate_boundaries.py"):
            with self.subTest(path=path):
                self.assertEqual(self.selected(path), set(changes.JOBS))
        self.assertTrue(all(changes.select_jobs(None).values()))


class GitDiffTests(unittest.TestCase):
    def test_pr_merge_base_push_range_rename_and_large_diff(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)

            def git(*args):
                return subprocess.check_output(["git", *args], cwd=root, stderr=subprocess.DEVNULL).decode().strip()

            git("init", "-b", "main")
            git("config", "user.name", "CI test")
            git("config", "user.email", "ci@example.invalid")
            (root / "old.rs").touch()
            git("add", "old.rs")
            git("commit", "-m", "base")
            base = git("rev-parse", "HEAD")
            git("checkout", "-b", "feature")
            (root / "old.rs").rename(root / "new.rs")
            for i in range(350):
                (root / f"file {i}.md").touch()
            git("add", "--all")
            git("commit", "-m", "feature")
            head = git("rev-parse", "HEAD")
            git("checkout", "main")
            (root / "base-only.rs").touch()
            git("add", "base-only.rs")
            git("commit", "-m", "advance base")
            advanced = git("rev-parse", "HEAD")
            pr = {"pull_request": {"base": {"sha": advanced}, "head": {"sha": head}}}
            paths = changes.changed_paths("pull_request", pr, head, root)
            self.assertEqual(len(paths), 352)
            self.assertIn("old.rs", paths)
            self.assertIn("new.rs", paths)
            self.assertIn("file 349.md", paths)
            self.assertNotIn("base-only.rs", paths)
            self.assertEqual(changes.changed_paths("push", {"before": base}, head, root), paths)
            self.assertIsNone(changes.changed_paths("push", {"before": "0" * 40}, head, root))
            self.assertIsNone(changes.changed_paths("workflow_dispatch", {}, head, root))
            self.assertIsNone(changes.changed_paths("push", {"before": "1" * 40}, head, root))


if __name__ == "__main__":
    unittest.main()
