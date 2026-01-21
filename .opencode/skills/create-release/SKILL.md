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

This skill guides you through creating a release for Annie Mei using trunk-based development. Releases are created by tagging commits on `main` and creating a GitHub release.

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

# Push the tag to trigger the release workflow
git push origin vX.X.X
```

The `build-release.yml` workflow will automatically:
- Build the ARM64 binary
- Create a GitHub release with the binary attached
- Deploy to Oracle Cloud

### Step 4: Wait for Workflow and Edit Release Notes

The `build-release.yml` workflow automatically creates the release. Wait for it to complete:

```bash
# Wait for and verify the workflow completed
gh run list --workflow=build-release.yml --limit=1

# View the created release
gh release view vX.X.X
```

Then edit the release notes to organize them into these sections:

```bash
gh release edit vX.X.X --notes "$(cat <<'EOF'
## Breaking Changes
- List any breaking changes (API changes, major upgrades)

## Improvements
- New features and enhancements
- New commands added

## Bug Fixes
- Bug fixes and corrections

## Dependencies
- Package updates with version changes (e.g., "Bump serde from 1.0.148 to 1.0.149")
EOF
)"
```

Or edit directly in the GitHub UI.

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

- The CI workflow triggers on tags matching `v[0-9]+.[0-9]+.[0-9]+`
- Releases are built on ARM64 runners for Oracle Cloud deployment
- The binary is validated before deployment; if validation fails, the deploy auto-rolls back
