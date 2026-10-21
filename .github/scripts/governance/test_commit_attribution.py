"""Exercise the validator through real Git commits in an isolated clone."""
from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("commit_attribution.py")


class AttributionTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="openhid-attribution-")
        self.addCleanup(self.tmp.cleanup)
        self.repo = Path(self.tmp.name)
        self.env = os.environ.copy()
        for key in list(self.env):
            if key.startswith("GIT_"):
                del self.env[key]
        self.env.update({
            "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_AUTHOR_NAME": "Fixture Maintainer",
            "GIT_AUTHOR_EMAIL": "maintainer@example.invalid",
            "GIT_COMMITTER_NAME": "Fixture Maintainer",
            "GIT_COMMITTER_EMAIL": "maintainer@example.invalid",
        })
        self.git("init", "-q")
        hooks = self.repo / ".git" / "hooks"
        shutil.copy2(SCRIPT, hooks / "attribution.py")
        # Git for Windows also executes hooks with its POSIX shell.
        python = sys.executable.replace("\\", "/")
        hook = hooks / "commit-msg"
        hook.write_text(f'#!/bin/sh\nexec "{python}" "$(dirname "$0")/attribution.py" "$1"\n', encoding="utf-8")
        hook.chmod(0o755)

    def git(self, *args):
        return subprocess.run(["git", "-C", str(self.repo), *args], env=self.env,
                              capture_output=True, text=True, encoding="utf-8", check=False)

    def commit(self, body):
        message = self.repo / "message.txt"
        message.write_text(body, encoding="utf-8")
        return self.git("-c", "commit.gpgsign=false", "commit", "--allow-empty", "-F", str(message))

    def test_actual_commits_keep_author_and_trailers(self):
        for trailer in (
            "Co-authored-by: Codex <codex@openai.com>",
            "Co-authored-by: Claude Code <noreply@anthropic.com>",
            "Co-authored-by: Cursor <cursoragent@cursor.com>",
            "Human-authored: true",
            "Co-authored-by: Contributor <contributor@example.invalid>\nHuman-authored: true",
            "Co-authored-by: Codex <codex@openai.com>\nCo-authored-by: Cursor <cursoragent@cursor.com>",
        ):
            with self.subTest(trailer=trailer):
                result = self.commit("test(ci): validate attribution\n\n" + trailer + "\n")
                self.assertEqual(result.returncode, 0, result.stderr)
                author = self.git("show", "-s", "--format=%an <%ae>", "HEAD")
                self.assertEqual(author.stdout.strip(), "Fixture Maintainer <maintainer@example.invalid>")
                self.assertIn(trailer, self.git("show", "-s", "--format=%B", "HEAD").stdout)

    def test_upstream_contributor_trailer_is_kept_in_final_trailer_block(self):
        message = (
            "fix(device): adopt upstream fix\n\n"
            "Imported change details.\n\n"
            "Co-authored-by: Contributor <contributor@example.invalid>\n\n"
            "Co-authored-by: Contributor <contributor@example.invalid>\n"
            "Co-authored-by: Codex <codex@openai.com>\n"
        )
        result = self.commit(message)
        self.assertEqual(result.returncode, 0, result.stderr)

        parsed = self.git("show", "-s", "--format=%B", "HEAD")
        trailers = self.git("interpret-trailers", "--parse", str(self.repo / "message.txt"))
        self.assertIn("Co-authored-by: Contributor <contributor@example.invalid>", trailers.stdout)
        self.assertIn("Co-authored-by: Codex <codex@openai.com>", trailers.stdout)
        self.assertIn("Imported change details.", parsed.stdout)

    def test_rejects_missing_forged_conflicting_and_body_attribution(self):
        for body in (
            "test(ci): missing attribution\n",
            "test(ci): invalid\n\nCo-authored-by: Codex <wrong@example.invalid>\n",
            "test(ci): conflict\n\nCo-authored-by: Codex <codex@openai.com>\nHuman-authored: true\n",
            "test(ci): invalid\n\nHuman-authored: false\n",
            "test(ci): body mention\n\nCo-authored-by: Codex <codex@openai.com>\n\nThis is prose, not a trailer.\n",
            "test(ci): contributor only\n\nCo-authored-by: Contributor <contributor@example.invalid>\n",
            "test(ci): hidden upstream author\n\n"
            "Co-authored-by: Contributor <contributor@example.invalid>\n\n"
            "Co-authored-by: Codex <codex@openai.com>\n",
        ):
            with self.subTest(body=body):
                result = self.commit(body)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("OpenHID:", result.stderr)
                self.assertNotEqual(self.git("rev-parse", "--verify", "HEAD").returncode, 0)

    def test_message_is_never_rewritten(self):
        for body in ("test(ci): no trailer\n", "test(ci): human\n\nHuman-authored: true\n"):
            self.commit(body)
            self.assertEqual((self.repo / "message.txt").read_text(encoding="utf-8"), body)


if __name__ == "__main__":
    unittest.main()
