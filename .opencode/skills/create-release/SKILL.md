---
name: create-release
description: Create a GitHub release with version tag and release notes for Annie Mei
license: MIT
compatibility: opencode
metadata:
  audience: maintainers
  workflow: github
---

## Overview

This skill guides you through creating a release for Annie Mei using trunk-based development. Releases are created by tagging commits on `main`, then manually running `build-release.yml` from `main` against the release tag so the build can reuse the Rust release cache warmed by normal `main` builds.

## Prerequisites

Before creating a release, verify:
1. You are on the `main` branch
2. The branch is up to date with `origin/main`
3. All CI checks are passing
4. The version in `Cargo.toml` has been bumped appropriately

## Release Process

### Step 1: Verify State

```bash
# Ensure you're on main and up to date
git checkout main
git pull origin main

# Check current version
grep '^version' Cargo.toml
```

### Step 2: Determine Version

Check the commits since the last release to determine the appropriate version bump:

```bash
# Find the last release tag
git describe --tags --abbrev=0

# See commits since last release
git log $(git describe --tags --abbrev=0)..HEAD --oneline
```

Apply semantic versioning:
- **MAJOR** (X.0.0): Breaking changes, incompatible API changes
- **MINOR** (0.X.0): New features, new commands, backwards-compatible functionality
- **PATCH** (0.0.X): Bug fixes, refactors, dependency updates

### Step 3: Create and Push Tag

```bash
# Create the tag (replace X.X.X with the version)
git tag vX.X.X

# Push the tag so GitHub Releases can point to it
git push origin vX.X.X
```

Pushing the tag does **not** trigger the release workflow. The workflow is dispatched manually from `main` in the next step to avoid cold tag-scoped Rust caches.

### Step 4: Run Release Build and Deploy

```bash
gh workflow run build-release.yml --ref main -f ref=vX.X.X
gh run watch
```

The `build-release.yml` workflow will:
- Check out the requested tag/ref
- Restore the main-scoped Rust release cache
- Validate that `vX.X.X` matches the `Cargo.toml` version
- Create or update the GitHub release and attach assets
- Create/upload Sentry release metadata and debug symbols
- Deploy to Oracle Cloud only for release refs that resolve to a `vX.Y.Z` tag

### Step 5: Edit Release Notes

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

You can edit via CLI or directly in the GitHub UI.

### Step 6: Verify Release

```bash
# Check the release was created with assets
gh release view vX.X.X

# Verify the workflow completed
gh run list --workflow=build-release.yml --limit=2
```

Also verify:
- The build logs show a Rust cache restore from the normal `main` build cache instead of `No cache found`
- Release assets are attached correctly
- Oracle deploy completed successfully
- `/healthz` passed in the workflow logs

## Rollback Procedure

If a release needs to be rolled back:

```bash
# Delete the release
gh release delete vX.X.X --yes

# Delete the tag (remote and local)
git push origin --delete vX.X.X
git tag -d vX.X.X
```

## Notes

- `main` pushes run `build-release.yml` as a cache warmer and do not create a release or deploy
- Releases are manual: `gh workflow run build-release.yml --ref main -f ref=vX.X.X`
- Releases are built on ARM64 runners for Oracle Cloud deployment
- Release runs restore cache but skip saving large caches on the critical deploy path
- The binary is validated before deployment; if validation fails, the deploy auto-rolls back
