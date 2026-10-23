"""Public CLI contract tests: real Git/SSH signing, fixture GitHub reads."""

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("preflight.py")
REPO = "repos/annie-mei/annie-mei"


class PreflightTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.env = {
            **os.environ,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}",
            "PREFLIGHT_FIXTURES": str(self.root / "api.json"),
        }
        # Isolate these generated-key fixtures from inherited Git overrides.
        for key in list(self.env):
            if key in {"GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR"} or key.startswith(
                ("GIT_CONFIG_COUNT", "GIT_CONFIG_KEY_", "GIT_CONFIG_VALUE_")
            ):
                del self.env[key]
        self.command("git", "init", "--quiet", "-b", "main")
        self.command("git", "config", "user.name", "Preflight Test")
        self.command("git", "config", "user.email", "preflight@example.test")
        self.key = self.root / "signing-key"
        self.command("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(self.key))
        self.fingerprint = self.command(
            "ssh-keygen", "-lf", str(self.key) + ".pub"
        ).split()[1]
        self.command("git", "config", "gpg.format", "ssh")
        self.command("git", "config", "user.signingkey", str(self.key))
        (self.repo / "Cargo.toml").write_text(
            '[package]\nname = "annie-mei"\nversion = "3.10.18"\n'
        )
        self.command("git", "add", "Cargo.toml")
        self.command("git", "commit", "--quiet", "-m", "fixture")
        self.commit = self.command("git", "rev-parse", "HEAD").strip()
        self.responses = {
            REPO: {"full_name": "annie-mei/annie-mei", "default_branch": "main"},
            f"{REPO}/git/ref/heads/main": {"object": {"sha": self.commit}},
            f"{REPO}/git/ref/tags/v3.10.18": {"error": 404},
            f"{REPO}/releases/tags/v3.10.18": {"error": 404},
            f"{REPO}/rulesets?includes_parents=true": [[]],
            "user": {"login": "test-owner"},
            "users/test-owner/ssh_signing_keys": [
                [{"key": Path(str(self.key) + ".pub").read_text().strip()}]
            ],
            f"{REPO}/actions/workflows/build-release.yml": {
                "id": 123,
                "state": "active",
                "path": ".github/workflows/build-release.yml",
            },
        }
        fake = self.bin / "gh"
        fake.write_text("""#!/usr/bin/env python3
import json, os, sys
assert sys.argv[1:4] == ["api", "--method", "GET"], "Only read-only API calls are permitted"
data = json.load(open(os.environ["PREFLIGHT_FIXTURES"]))[sys.argv[4]]
if isinstance(data, dict) and "error" in data:
    print(f"gh: PRIVATE_ERROR_SENTINEL (HTTP {data['error']})", file=sys.stderr)
    sys.exit(1)
print(json.dumps(data))
""")
        fake.chmod(0o755)

    def command(self, *args):
        return subprocess.run(
            args,
            cwd=self.repo,
            env=self.env,
            text=True,
            capture_output=True,
            check=True,
        ).stdout

    def preflight(self, expected, overrides=None):
        (self.root / "api.json").write_text(json.dumps(self.responses))
        config = (self.repo / ".git/config").read_bytes()
        refs = self.command("git", "show-ref")
        result = subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "v3.10.18",
                "--commit",
                self.commit,
                "--approved-key",
                self.fingerprint,
            ],
            cwd=self.repo,
            env={**self.env, **(overrides or {})},
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(
            result.returncode,
            {"ready": 0, "blocked": 1, "unknown": 2}[expected],
            result.stdout + result.stderr,
        )
        report = json.loads(result.stdout)
        self.assertEqual(report["status"], expected)
        self.assertEqual((self.repo / ".git/config").read_bytes(), config)
        self.assertEqual(self.command("git", "show-ref"), refs)
        self.assertEqual(list((self.repo / ".git").glob("release-preflight-*")), [])
        self.assertNotIn("PRIVATE_ERROR_SENTINEL", result.stdout + result.stderr)
        return {check["name"]: check for check in report["checks"]}

    def test_ready_requires_a_real_approved_tag_signature(self):
        checks = self.preflight("ready")
        self.assertEqual(checks["tag_signing"]["fingerprint"], self.fingerprint)
        self.assertEqual(checks["source"]["commit"], self.commit)

    def test_existing_tag_or_release_blocks_without_replacement(self):
        for path in (f"{REPO}/git/ref/tags/v3.10.18", f"{REPO}/releases/tags/v3.10.18"):
            with self.subTest(path=path):
                self.responses[path] = {"object": {"sha": self.commit}}
                self.assertEqual(
                    self.preflight("blocked")["new_tag"]["status"], "blocked"
                )
                self.responses[path] = {"error": 404}
        self.command("git", "tag", "v3.10.18")
        self.assertEqual(self.preflight("blocked")["new_tag"]["status"], "blocked")

    def test_denied_or_unavailable_reads_are_unknown_not_absent(self):
        for path in (
            f"{REPO}/git/ref/tags/v3.10.18",
            f"{REPO}/rulesets?includes_parents=true",
            "users/test-owner/ssh_signing_keys",
            f"{REPO}/actions/workflows/build-release.yml",
        ):
            original = self.responses[path]
            for status in (403, 500):
                with self.subTest(path=path, status=status):
                    self.responses[path] = {"error": status}
                    self.preflight("unknown")
            self.responses[path] = original

    def test_tag_restrictions_apply_despite_admin_bypass_and_pagination(self):
        self.responses[f"{REPO}/rulesets?includes_parents=true"] = [
            [],
            [{"id": 42, "target": "tag", "enforcement": "active"}],
        ]
        rule = {
            "id": 42,
            "conditions": {
                "ref_name": {
                    "include": ["refs/tags/v*", "refs/tags/v[0-9]+.[0-9]+.[0-9]+"],
                    "exclude": [],
                }
            },
            "rules": [{"type": "creation"}],
            "current_user_can_bypass": "always",
        }
        self.responses[f"{REPO}/rulesets/42"] = rule
        self.assertEqual(self.preflight("blocked")["tag_rules"]["rulesets"], [42])
        rule["conditions"]["ref_name"]["exclude"] = ["refs/tags/v3.10.18"]
        self.preflight("ready")
        rule["conditions"]["ref_name"]["exclude"] = ["refs/tags/v3.10.19"]
        self.preflight("blocked")
        for pattern in ("refs/tags/**", "refs/tags/v[^4]*", "refs/tags/v[[:digit:]]*"):
            with self.subTest(pattern=pattern):
                rule["conditions"]["ref_name"]["include"] = [pattern]
                self.preflight("unknown")

    def test_git_environment_cannot_redirect_the_probe_to_real_refs(self):
        for name in ("GIT_DIR", "GIT_COMMON_DIR", "GIT_OBJECT_DIRECTORY"):
            with self.subTest(name=name):
                checks = self.preflight("unknown", {name: str(self.repo / ".git")})
                self.assertEqual(checks["source"]["status"], "unknown")

    def test_disabled_workflow_blocks(self):
        self.responses[f"{REPO}/actions/workflows/build-release.yml"]["state"] = (
            "disabled_manually"
        )
        self.assertEqual(
            self.preflight("blocked")["workflow_access"]["status"], "blocked"
        )

    def test_registered_key_is_not_enough_if_signer_fails_or_uses_another_key(self):
        failing = self.bin / "failing-signer"
        failing.write_text("#!/bin/sh\necho PRIVATE_ERROR_SENTINEL >&2\nexit 1\n")
        failing.chmod(0o755)
        self.command("git", "config", "gpg.ssh.program", str(failing))
        self.assertEqual(self.preflight("blocked")["tag_signing"]["status"], "blocked")
        self.command("git", "config", "--unset", "gpg.ssh.program")
        other = self.root / "other-key"
        self.command("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(other))
        self.command("git", "config", "user.signingkey", str(other))
        self.assertEqual(self.preflight("blocked")["tag_signing"]["status"], "blocked")

    def test_unregistered_key_and_existing_hooks_fail_closed(self):
        keys = self.responses["users/test-owner/ssh_signing_keys"]
        self.responses["users/test-owner/ssh_signing_keys"] = [[]]
        self.assertIn("registered", self.preflight("blocked")["tag_signing"]["reason"])
        self.responses["users/test-owner/ssh_signing_keys"] = keys
        hook = self.repo / ".git/hooks/reference-transaction"
        hook.write_text("#!/bin/sh\nexit 1\n")
        hook.chmod(0o755)
        self.assertEqual(self.preflight("blocked")["tag_signing"]["status"], "blocked")

    def test_source_version_and_remote_main_must_match(self):
        self.responses[f"{REPO}/git/ref/heads/main"] = {"object": {"sha": "a" * 40}}
        self.assertEqual(self.preflight("blocked")["source"]["status"], "blocked")
        self.responses[f"{REPO}/git/ref/heads/main"] = {"object": {"sha": self.commit}}
        (self.repo / "Cargo.toml").write_text(
            '[package]\nname = "annie-mei"\nversion = "3.10.19"\n'
        )
        self.command("git", "commit", "--quiet", "-am", "different version")
        self.commit = self.command("git", "rev-parse", "HEAD").strip()
        self.responses[f"{REPO}/git/ref/heads/main"] = {"object": {"sha": self.commit}}
        self.assertEqual(self.preflight("blocked")["source"]["status"], "blocked")


if __name__ == "__main__":
    unittest.main()
