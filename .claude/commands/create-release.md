Create a GitHub release for Annie Mei using trunk-based development. Follow every step below, confirming with the user before destructive actions (tagging, pushing).

## Step 1: Verify State

1. Confirm you are on the `main` branch and it is up to date with `origin/main` (run `git fetch origin` and compare).
2. Read the current version from `Cargo.toml`.
3. Confirm CI is green on the latest commit: `gh run list --branch main --limit 1`.

If any check fails, stop and tell the user what needs fixing.

## Step 2: Determine Version

1. Find the last release tag: `git describe --tags --abbrev=0`
2. List commits since that tag: `git log $(git describe --tags --abbrev=0)..HEAD --oneline`
3. Apply semantic versioning based on the commits:
   - **MAJOR** (X.0.0): Breaking changes, incompatible API changes
   - **MINOR** (0.X.0): New features, new commands, backwards-compatible functionality
   - **PATCH** (0.0.X): Bug fixes, refactors, dependency updates
4. Verify the version in `Cargo.toml` matches the intended release version. If it doesn't, stop and ask the user — the version bump should already be committed to `main`.

## Step 3: Create and Push Tag

Ask the user to confirm the version before proceeding.

```bash
git tag vX.X.X
git push origin vX.X.X
```

## Step 4: Create GitHub Release

Create the release with auto-generated notes:

```bash
gh release create vX.X.X --generate-notes
```

The `build-release.yml` workflow will automatically:
- Build the ARM64 binary
- Upload debug symbols to Sentry
- Attach the binary to the release
- Deploy to Oracle Cloud

## Step 5: Edit Release Notes

Edit the release notes to organize them into these sections using `gh release edit`:

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

Remove any section that has no entries.

## Step 6: Verify Release

```bash
gh release view vX.X.X
gh run list --workflow=build-release.yml --limit=1
```

Wait for the workflow to complete and report the result. If it fails, alert the user with the failure details.

## Rollback (only if the user asks)

```bash
gh release delete vX.X.X --yes
git push origin --delete vX.X.X
git tag -d vX.X.X
```
