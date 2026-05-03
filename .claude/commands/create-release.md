Create a GitHub release for Annie Mei using trunk-based development. Follow every step below, confirming with the user before destructive actions (tagging, pushing).

Releases are created by tagging commits on `main`, then manually running `build-release.yml` from `main` against the release tag so the build can reuse the Rust release cache warmed by normal `main` builds.

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

Pushing the tag does **not** trigger the release workflow. The workflow is dispatched manually from `main` in the next step to avoid cold tag-scoped Rust caches.

## Step 4: Run Release Build and Deploy

```bash
gh workflow run build-release.yml --ref main -f ref=vX.X.X
gh run watch
```

The `build-release.yml` workflow will:
- Check out the requested tag/ref
- Restore the Rust release cache warmed by normal `main` builds
- Validate that `vX.X.X` matches the `Cargo.toml` version
- Create or update the GitHub release and attach assets
- Create/upload Sentry release metadata and debug symbols
- Deploy to Oracle Cloud only for release refs that resolve to a `vX.Y.Z` tag

## Step 5: Edit Release Notes

Edit the generated release notes to organize them into these sections using `gh release edit`:

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
gh run list --workflow=build-release.yml --limit=2
```

Wait for the workflow to complete and report the result. Verify:
- The build logs show a Rust cache restore from the normal `main` build cache instead of `No cache found`
- Release assets are attached correctly
- Oracle deploy completed successfully
- `/healthz` passed in the workflow logs

If the workflow fails, alert the user with the failure details.

## Rollback (only if the user asks)

```bash
gh release delete vX.X.X --yes
git push origin --delete vX.X.X
git tag -d vX.X.X
```
