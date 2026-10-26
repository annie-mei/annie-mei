#!/usr/bin/env python3
"""Check a new Annie Mei release without changing remote state or real tags."""

import argparse
import fnmatch
import json
import os
import re
import subprocess
import tempfile
import tomllib
from datetime import UTC, datetime
from pathlib import Path

REPOSITORY = "annie-mei/annie-mei"


class Unknown(Exception):
    pass


def run(*args, input=None, env=None):
    try:
        return subprocess.run(
            args,
            input=input,
            text=True,
            capture_output=True,
            timeout=60,
            env=env,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        # Do not expose command arguments, credentials, or raw service errors.
        raise Unknown(f"{args[0]} unavailable or timed out") from error


def git(*args):
    result = run("git", *args)
    if result.returncode:
        raise Unknown("Git inspection failed")
    return result.stdout.strip()


def api(path, *, collection=False, missing=False):
    args = ["gh", "api", "--method", "GET", path]
    if collection:
        args += ["--paginate", "--slurp"]
    result = run(*args)
    if result.returncode:
        if missing and re.search(r"\bHTTP 404\b", result.stderr):
            return None
        raise Unknown("GitHub read failed; absence or permission cannot be inferred")
    try:
        data = json.loads(result.stdout)
        return [item for page in data for item in page] if collection else data
    except (ValueError, TypeError) as error:
        raise Unknown("Unexpected GitHub response") from error


def matches(patterns, tag):
    """Support tag-only globs; fail closed on other GitHub pattern syntax."""
    unsupported = False
    for pattern in patterns:
        if pattern == "~ALL":
            return True
        if not pattern.startswith("refs/tags/") or "**" in pattern or "\\" in pattern:
            unsupported = True
            continue
        # Release names cannot contain '/', so these basename globs do not cross paths.
        suffix = pattern.removeprefix("refs/tags/")
        # Ruby/GitHub and Python differ on extended/negated character classes.
        # Support the numeric range used by this repo, not an approximation of all glob syntax.
        if not re.fullmatch(r"[A-Za-z0-9.*?+_-]+", suffix.replace("[0-9]", "0")):
            unsupported = True
            continue
        if fnmatch.fnmatchcase(tag, suffix):
            return True
    if unsupported:
        raise Unknown("Tag ruleset uses an unsupported pattern; inspect it manually")
    return False


def check_rules(tag):
    restricted = []
    unknowns = []
    for summary in api(
        f"repos/{REPOSITORY}/rulesets?includes_parents=true", collection=True
    ):
        try:
            if summary["target"] != "tag" or summary["enforcement"] != "active":
                continue
            rule = api(f"repos/{REPOSITORY}/rulesets/{summary['id']}")
            conditions = rule["conditions"]["ref_name"]
            if not matches(conditions["include"], tag) or matches(
                conditions["exclude"], tag
            ):
                continue
            types = {entry["type"] for entry in rule["rules"]}
            if "creation" in types:
                # A configured admin exemption is not permission to bypass the rule.
                restricted.append(rule["id"])
            if types - {"creation", "update", "deletion"}:
                raise Unknown("Additional tag rules require manual evaluation")
        except (Unknown, KeyError, TypeError, ValueError) as error:
            # Keep inspecting: uncertainty must not erase known creation blockers,
            # regardless of the order in which GitHub returns rulesets.
            unknowns.append(
                str(error)
                if isinstance(error, Unknown)
                else "Unexpected ruleset format"
            )
    if restricted:
        return "blocked", {
            "reason": "Tag creation is restricted, even for bypass actors",
            "rulesets": restricted,
            "unknowns": unknowns,
        }
    if unknowns:
        return "unknown", {
            "reason": "Tag rules require manual evaluation",
            "unknowns": unknowns,
        }
    return "ready", {"reason": "No applicable active tag-creation restriction found"}


def check_signing(commit, tag, approved):
    if git("config", "--get", "gpg.format") != "ssh":
        raise Unknown("This preflight supports SSH signing only")
    login = api("user")["login"]
    trusted = []
    for key in api(f"users/{login}/ssh_signing_keys", collection=True):
        fingerprint = run("ssh-keygen", "-lf", "-", input=key["key"] + "\n")
        if fingerprint.returncode:
            raise Unknown("Could not fingerprint a registered signing key")
        if fingerprint.stdout.split()[1] in approved:
            trusted.append(key["key"])
    if not trusted:
        return "blocked", {
            "reason": "No approved fingerprint is registered as a GitHub signing key"
        }

    git_dir = Path(git("rev-parse", "--absolute-git-dir"))
    objects = git("rev-parse", "--path-format=absolute", "--git-path", "objects")
    config = []
    for name in (
        "user.name",
        "user.email",
        "user.signingkey",
        "gpg.format",
        "gpg.program",
        "gpg.ssh.program",
        "gpg.ssh.defaultKeyCommand",
        "gpg.ssh.revocationFile",
        "gpg.minTrustLevel",
    ):
        value = run("git", "config", "--get", name)
        if value.returncode == 0:
            config += ["-c", f"{name}={value.stdout.rstrip(chr(10))}"]
        elif value.returncode != 1:
            raise Unknown("Could not inspect effective signing configuration")
    hooks = run("git", "config", "--path", "--get", "core.hooksPath")
    if hooks.returncode not in (0, 1):
        raise Unknown("Could not inspect hook configuration")
    common_dir = Path(git("rev-parse", "--path-format=absolute", "--git-common-dir"))
    hooks_path = (
        Path(hooks.stdout.strip()).resolve()
        if hooks.returncode == 0
        else common_dir / "hooks"
    )
    config += ["-c", f"core.hooksPath={hooks_path}"]

    # Keep the probe under the real Git directory so path-scoped signing config
    # still applies. Reuse objects read-only, but keep all new objects/refs isolated.
    with tempfile.TemporaryDirectory(
        prefix="release-preflight-", dir=git_dir
    ) as directory:
        probe = Path(directory)
        initialized = run("git", "init", "--bare", "--quiet", str(probe / "repo.git"))
        if initialized.returncode:
            raise Unknown("Could not initialize disposable signing probe")
        command = ["git", f"--git-dir={probe / 'repo.git'}", *config]
        env = {**os.environ, "GIT_ALTERNATE_OBJECT_DIRECTORIES": objects, "LC_ALL": "C"}
        signed = run(
            *command, "tag", "-s", tag, commit, "-m", f"Release {tag}", env=env
        )
        if signed.returncode:
            return "blocked", {
                "reason": "Configured signer could not sign an annotated tag; do not retry or substitute keys"
            }
        allowed = probe / "allowed-signers"
        allowed.write_text("".join(f"release-preflight {key}\n" for key in trusted))
        verified = run(
            *command,
            "-c",
            f"gpg.ssh.allowedSignersFile={allowed}",
            "verify-tag",
            tag,
            env=env,
        )
        if verified.returncode:
            return "blocked", {
                "reason": "Tag signature does not verify against approved GitHub signing keys"
            }
        fingerprint = re.search(
            r"SHA256:[A-Za-z0-9+/]+", verified.stdout + verified.stderr
        )
        if not fingerprint or fingerprint[0] not in approved:
            raise Unknown("Verified tag signer fingerprint could not be established")
        return "ready", {
            "fingerprint": fingerprint[0],
            "reason": "Disposable annotated tag signed and verified",
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag", help="New vX.Y.Z release tag")
    parser.add_argument("--commit", required=True, help="Full approved commit SHA")
    parser.add_argument(
        "--approved-key",
        action="append",
        required=True,
        help="Owner-approved SSH SHA256 fingerprint; repeatable",
    )
    args = parser.parse_args()
    if not re.fullmatch(r"v\d+\.\d+\.\d+", args.tag) or not re.fullmatch(
        r"[0-9a-f]{40}", args.commit
    ):
        parser.error("Use a vX.Y.Z tag and a full lowercase commit SHA")
    if any(
        not re.fullmatch(r"SHA256:[A-Za-z0-9+/]{43}", key) for key in args.approved_key
    ):
        parser.error("Approved keys must be SHA256 SSH fingerprints")

    checks = []

    def check(name, action):
        try:
            status, evidence = action()
        except (Unknown, KeyError, TypeError, ValueError, OSError) as error:
            status = "unknown"
            evidence = {
                "reason": str(error)
                if isinstance(error, Unknown)
                else "Unexpected evidence format"
            }
        checks.append({"name": name, "status": status, **evidence})
        return status == "ready"

    def source():
        if any(
            os.environ.get(name)
            for name in (
                "GIT_DIR",
                "GIT_WORK_TREE",
                "GIT_COMMON_DIR",
                "GIT_OBJECT_DIRECTORY",
                "GIT_INDEX_FILE",
                "GIT_NAMESPACE",
            )
        ):
            raise Unknown(
                "Git directory/environment overrides require manual inspection; probe not run"
            )
        if git("rev-parse", "--verify", f"{args.commit}^{{commit}}") != args.commit:
            return "blocked", {"reason": "Requested commit does not resolve exactly"}
        # Git expands insteadOf/pushInsteadOf and includes separate push URLs.
        # Inspect every destination without exposing potentially credentialed URLs.
        destinations = git(
            "remote", "get-url", "--push", "--all", "origin"
        ).splitlines()
        if not destinations or any(
            not re.fullmatch(
                rf"(?:https://github\.com/|ssh://git@github\.com/|git@github\.com:)"
                rf"{re.escape(REPOSITORY)}(?:\.git)?/?",
                destination,
            )
            for destination in destinations
        ):
            return "blocked", {
                "reason": "Every effective origin push URL must target the checked GitHub repository"
            }
        cargo = tomllib.loads(git("show", f"{args.commit}:Cargo.toml"))
        version = cargo["package"]["version"]
        if cargo["package"]["name"] != "annie-mei" or args.tag != f"v{version}":
            return "blocked", {
                "reason": "Tag does not match the pinned Annie Mei Cargo version",
                "version": version,
            }
        lock = tomllib.loads(git("show", f"{args.commit}:Cargo.lock"))
        versions = [
            package["version"]
            for package in lock["package"]
            if package["name"] == "annie-mei"
        ]
        if versions != [version]:
            return "blocked", {
                "reason": "Pinned Cargo.lock must contain one Annie Mei package matching Cargo.toml",
                "version": version,
            }
        repository = api(f"repos/{REPOSITORY}")
        if (
            repository["full_name"] != REPOSITORY
            or repository["default_branch"] != "main"
        ):
            return "blocked", {"reason": "Unexpected repository or default branch"}
        head = api(f"repos/{REPOSITORY}/git/ref/heads/main")["object"]["sha"]
        if head != args.commit:
            return "blocked", {
                "reason": "Remote main differs from the approved commit",
                "main": head,
            }
        return "ready", {
            "repository": REPOSITORY,
            "commit": args.commit,
            "version": version,
            "tag": args.tag,
        }

    def new_tag():
        local = run("git", "show-ref", "--verify", "--quiet", f"refs/tags/{args.tag}")
        if local.returncode == 0:
            return "blocked", {
                "reason": "Local tag already exists; reuse or replacement requires a separate plan"
            }
        if local.returncode != 1:
            raise Unknown("Could not inspect local tag")
        if api(f"repos/{REPOSITORY}/git/ref/tags/{args.tag}", missing=True) is not None:
            return "blocked", {
                "reason": "Remote tag already exists; never replace it automatically"
            }
        if (
            api(f"repos/{REPOSITORY}/releases/tags/{args.tag}", missing=True)
            is not None
        ):
            return "blocked", {"reason": "GitHub release already exists"}
        return "ready", {"reason": "No local tag, remote tag, or GitHub release exists"}

    def workflow():
        data = api(f"repos/{REPOSITORY}/actions/workflows/build-release.yml")
        if (
            data["state"] != "active"
            or data["path"] != ".github/workflows/build-release.yml"
        ):
            return "blocked", {
                "reason": "Release workflow is not active at the expected path"
            }
        return "ready", {
            "workflow_id": data["id"],
            "reason": "Release workflow is visible and active; dispatch permission is not proven",
        }

    if check("source", source):
        if check("new_tag", new_tag) and check(
            "tag_rules", lambda: check_rules(args.tag)
        ):
            check(
                "tag_signing",
                lambda: check_signing(args.commit, args.tag, args.approved_key),
            )
        check("workflow_access", workflow)
    status = (
        "blocked"
        if any(c["status"] == "blocked" for c in checks)
        else ("unknown" if any(c["status"] == "unknown" for c in checks) else "ready")
    )
    print(
        json.dumps(
            {
                "scope": "new-release-git-and-github-preflight",
                "status": status,
                "checks": checks,
                "checked_at": datetime.now(UTC).isoformat(),
                "remaining_checks": [
                    "Exact-commit CI and review evidence",
                    "Sentry organization/project access and pre-deploy error baseline",
                    "Recorded authorization for tag push, release publication and production workflow dispatch (eligible factory grant or specific owner approval)",
                ],
                "limitations": [
                    "Read access does not prove dispatch/write permission or deployment secret validity",
                    "Snapshot only: recheck main, tag absence and rules immediately before an authorized push",
                    "No remote changes, fetch, release, or workflow dispatch was performed",
                ],
            },
            indent=2,
        )
    )
    return {"ready": 0, "blocked": 1, "unknown": 2}[status]


if __name__ == "__main__":
    raise SystemExit(main())
