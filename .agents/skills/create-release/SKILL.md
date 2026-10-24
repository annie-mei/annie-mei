---
name: create-release
description: Checks release prerequisites and guides authorized Annie Mei releases. Use when preparing, publishing, or verifying a release.
license: MIT
compatibility: Requires Python 3.11+, Git with SSH signing, ssh-keygen, and the GitHub CLI (gh)
metadata:
  audience: maintainers
  workflow: github
---

## Overview

This skill guides you through creating a release for Annie Mei using trunk-based development. Releases are created by tagging commits on `main`, then manually running `build-release.yml` from `main` against the release tag so the build can reuse the Rust release cache warmed by normal `main` builds.

## Phase 1: Preflight (no remote mutations)

Do not fetch, switch branches, update versions, create real tags, publish releases,
dispatch workflows, or change signing configuration/protection during preflight.
Preserve existing work. Version changes belong on an authorized ticket branch,
not on `main`. Apply semantic versioning before preparing the release candidate:
major for breaking changes, minor for compatible features, patch for fixes/refactors.

### Pin the candidate and approved signer

Identify the owner-approved full commit SHA, `vX.Y.Z` tag, and allowed SSH signing
fingerprint(s). Never infer approval from the keys available on a machine. Inspect
remote `main` with `gh api repos/annie-mei/annie-mei/git/ref/heads/main`; a local
`origin/main` may be stale. If the commit is missing locally, stop and request
scoped fetch authorization rather than pulling or changing branches.

Run from the repository root, substituting the approved values:

```bash
python3 .agents/skills/create-release/scripts/preflight.py vX.Y.Z \
  --commit FULL_APPROVED_COMMIT_SHA \
  --approved-key 'SHA256:APPROVED_SIGNING_FINGERPRINT'
```

Repeat `--approved-key` only for additional owner-approved keys. The checker is
for **new releases**: any existing local tag, remote tag, or GitHub release blocks
this path. Reusing an existing release requires a separate explicit plan; do not
delete or overwrite anything to make preflight pass.

The checker requires every effective `origin` push URL (including separate push
URLs and Git URL rewrites) to target `annie-mei/annie-mei` on GitHub. Standard
HTTPS and Git SSH URLs are supported; aliases and other transports fail closed.
It pins `Cargo.toml` and the matching `annie-mei` package in `Cargo.lock` to the
commit, compares remote `main`, checks tag absence and active inherited/repository
tag rules, and checks workflow visibility.
If those prerequisites permit creating a tag, it signs and verifies an actual
annotated tag in a temporary Git directory using the effective configured signer
and existing hooks. The temporary tag is removed, never pushed. A signing prompt
may require the owner; a signed commit alone is not proof of tag-signing support.
Only owner-approved fingerprints registered as **signing keys** on the authenticated
GitHub account are trusted; authentication keys alone do not qualify.

The JSON report has scope `new-release-git-and-github-preflight` and exit codes:

| Status | Exit | Meaning |
| --- | --- | --- |
| `ready` | 0 | The automated Git/GitHub checks passed; remaining checks below still apply |
| `blocked` | 1 | A known prerequisite fails; report it and stop |
| `unknown` | 2 | Evidence is unavailable or unsupported; do not treat it as passing |

Unsupported rules/signers and denied APIs fail closed. An admin exemption does
not make a creation restriction pass. Known creation restrictions remain `blocked`
even when other rules are unknown; the report retains both findings.
Never substitute a signer, register a key,
disable signing/hooks, retry a failed Git operation, or relax a rule automatically.

### Complete the release gate

1. Inspect CI and required review evidence for the **exact pinned commit**. Do not
   confuse successful checks with enforced checks or skip unresolved blockers.
2. Through the existing Sentry connection, verify access to organization
   `annie-mei`, project `annie-mei`. Record a timestamped pre-deploy error baseline
   and the expected release `annie-mei@X.Y.Z`. No errors is not proof of traffic or
   health. Missing/denied telemetry is `unknown`; never print credentials or raw
   connection strings, and do not add a new token just to complete this check.
3. Read the release workflow at the pinned revision and confirm the intended
   release/deployment effects. Workflow visibility does **not** establish dispatch
   permission, validity of deployment secrets, or future service readiness. Record
   these untested capabilities explicitly as `unknown`; do not probe by dispatching,
   restarting production, or relaxing controls.
4. Report repository, commit, version, tag, signer fingerprint, checks with their
   evidence/time, remaining unknowns, and current authorization. Overall preflight
   is `blocked` if any check fails, otherwise `unknown` if required evidence is
   missing, otherwise `ready`. Stop for unresolved blockers/unknowns. The owner may
   explicitly authorize proceeding with a named untestable capability; retain that
   limitation in the report rather than relabeling it verified.

Permission to inspect is not permission to publish. Require explicit authorization
for tag push, release publication/edit, workflow dispatch, and production deployment
unless already unambiguously granted for this candidate. Preflight does not enforce
GitHub access controls or install an unattended factory executor.

## Phase 2: Authorized execution

### Create and push the signed tag

Release tags should be signed using the same configured signing mechanism as commits. For this repo, commits and tags use SSH signing via `gpg.format=ssh` and `user.signingkey`.

```bash
# Recheck remote main, tag absence and protection before this authorized mutation.
# Pin the target explicitly; do not tag the current branch's HEAD by accident.
git tag -s vX.X.X FULL_APPROVED_COMMIT_SHA -m "Release vX.X.X"
git verify-tag vX.X.X
```

Stop on either command's failure. Verify the actual tag's signer against the
approved fingerprint, not merely any locally trusted key, and confirm the peeled
commit equals the approved SHA. Only then perform the authorized push:

```bash
# Reconfirm origin's effective push URLs still target the checked repository.
git push origin vX.X.X
```

Do **not** create release tags with plain `git tag vX.X.X` when `tag.gpgsign=true`; Git may open the configured editor for the required annotated tag message. Always provide `-m "Release vX.X.X"` to keep the process non-interactive while preserving signed tags.

Pushing the tag does **not** trigger the release workflow. The workflow is dispatched manually from `main` in the next step to avoid cold tag-scoped Rust caches.

After push, verify the remote annotated object, peeled commit,
and GitHub signature status. If any differs or is unverified, stop before dispatch;
do not recreate the tag as an automatic recovery step.

### Run Release Build and Deploy

```bash
gh workflow run build-release.yml --repo github.com/annie-mei/annie-mei --ref main -f ref=vX.X.X
gh run watch EXACT_DISPATCHED_RUN_ID --repo github.com/annie-mei/annie-mei --exit-status
```

Capture the run URL/ID returned by dispatch. If unavailable, reconcile runs against
the dispatch time, event, ref, and SHA; ambiguity must stop, not trigger a second
dispatch. A successful `main` cache-warmer run is not a release deployment.

The `build-release.yml` workflow will:
- Check out the requested tag/ref
- Restore the main-scoped Rust release cache
- Validate that `vX.X.X` matches the `Cargo.toml` version
- Create or update the GitHub release and attach assets
- Create/upload Sentry release metadata and debug symbols
- Deploy to Oracle Cloud only for release refs that resolve to a `vX.Y.Z` tag

### Edit Release Notes

Edit the generated release notes to organize them into these sections:

```markdown
## Breaking Changes
- List any breaking changes (API changes, major upgrades)

## Improvements
- New features and enhancements
- New commands added

## Bug Fixes
- Bug fixes and corrections

## Dependencies
- Package updates with version changes (e.g., "Bump serde from 1.0.148 to 1.0.149")
```

Read the current release notes before an authorized edit; preserve others' edits.
You can edit via CLI or directly in the GitHub UI.

### Verify Release

```bash
# Check the release was created with assets
gh release view vX.X.X --repo github.com/annie-mei/annie-mei

# Verify the workflow completed
gh run view EXACT_DISPATCHED_RUN_ID --repo github.com/annie-mei/annie-mei
```

Also verify:
- The build logs show a Rust cache restore from the normal `main` build cache instead of `No cache found`
- Release assets are attached correctly
- Oracle deploy completed successfully
- `/healthz` passed in the workflow logs (this is the companion auth service,
  not proof of bot readiness)
- Sentry release creation and symbol upload succeeded, including steps marked
  `continue-on-error`; disclose gaps rather than relying on the overall green run

## Rollback Procedure

Rollback, tag replacement/deletion, and protection changes require separate explicit
approval. Deleting a GitHub release or tag does not roll back a running service.
Stop and propose a recovery plan tied to the previous known-good deployment and
any data-compatibility constraints; do not use destructive cleanup as a retry.

## Validate this skill's checker

```bash
python3 .agents/skills/create-release/scripts/test_preflight.py -v
```

Tests use disposable repositories and generated SSH keys with fixture GitHub reads.
They do not require personal credentials or change real tags or remote state.
The same command runs in `scripts/verify.sh` and the Cargo Test CI workflow.

## Notes

- `main` pushes run `build-release.yml` as a cache warmer and do not create a release or deploy
- Releases are manual: `gh workflow run build-release.yml --repo github.com/annie-mei/annie-mei --ref main -f ref=vX.X.X`
- Releases are built on ARM64 runners for Oracle Cloud deployment
- Release runs restore cache but skip saving large caches on the critical deploy path
- The binary is validated before deployment; if validation fails, the deploy auto-rolls back
