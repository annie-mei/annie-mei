---
name: pull-request
description: "Prepares and manages Annie Mei pull requests. Use when asked to create, update, inspect readiness, comment on, review, merge, or close a PR."
compatibility: Requires git, jq, and the GitHub CLI (gh)
metadata:
  audience: maintainers
  workflow: github
---

# Pull Request

Use this as the canonical agent workflow for Annie Mei pull requests.

## Authorization Boundary

Treat these as separate shared-state actions: **open**, **update**, **comment**, **review**, **merge**, and **close**. Perform only the specific action the user explicitly authorized. Permission for one action never implies another; implementation, commit, push, or readiness requests do not authorize any PR mutation. Read-only inspection is allowed. Pushing also requires explicit authorization under repository Git safety rules.

Before a mutation, state the exact action and confirm the request authorizes it. If ambiguous, stop and ask.

## Prepare

1. Read `AGENTS.md` and identify the Linear ticket and its suggested branch.
2. Inspect `git status --short --branch`; never operate from `main` and never discard unrelated changes.
3. Fetch the base branch and establish the exact PR range. For an existing PR, read its state:

   ```bash
   gh pr view <number-or-url> --json number,url,title,body,baseRefName,headRefName,headRefOid,isDraft,mergeable,mergeStateStatus,reviewDecision,statusCheckRollup
   git fetch origin <baseRefName>
   ```

4. Inspect the entire change and commit sequence against the base:

   ```bash
   git diff --stat origin/<baseRefName>...HEAD
   git diff origin/<baseRefName>...HEAD
   git log --reverse --format='%H%x09%s' origin/<baseRefName>..HEAD
   ```

5. Select validation according to the changed behavior, not a fixed checklist:
   - Documentation or agent guidance: `git diff --check` plus focused structural/content checks.
   - Rust changes: `cargo fmt --check`, targeted tests, `cargo test --all-features`, and `cargo clippy --all-features` as warranted by scope.
   - Discord behavior: exercise representative command paths and include Discord QA when runtime credentials are required.
   - Database/auth/release changes: run the owning subsystem's checks and identify manual health, migration, or rollout verification.
   Report skipped or unavailable checks honestly.

## Compose or Update

- Use `[ANNIE-<ticket-number>]/<description>` for the title.
- Read and fill [`reference/description-template.md`](reference/description-template.md). Remove placeholder comments and unused optional sections.
- Build `## Changes` from the complete base-to-head commit range. Use one bullet per commit in chronological order, with the full 40-character SHA and a concise description; do not wrap SHAs in code formatting.
- Check only type and validation boxes supported by the actual diff and executed evidence.
- Name the actual model in the footer; never leave `MODEL_NAME`.
- Do not mention the Linear ticket in the body solely to create a link; branch automation supplies it. Include external references only when useful to reviewers.
- Before an authorized open/update, re-read the final title/body, confirm the branch and base, and ensure no credentials or raw user IDs appear.

## Review Findings

When reviewing a PR, responding to findings, or checking readiness, follow [`reference/review-threads.md`](reference/review-threads.md). It is mandatory to:

1. Retrieve every review thread and every comment via paginated GraphQL, including resolved and outdated threads.
2. Record the PR's current `headRefOid`.
3. Verify each finding against code at that exact head SHA. Never treat comment position, resolution, or age as proof that the finding still applies.
4. Re-read the PR head after verification. If the SHA changed, repeat verification against the new head before reporting or mutating state.

## Readiness

Readiness is a read-only assessment, not authorization to update, review, merge, or close. Immediately before reporting readiness:

1. Confirm the assessed local or fetched commit equals the PR's current `headRefOid`.
2. Review the complete base-to-head diff and commit list.
3. Complete review-thread retrieval and current-head finding verification.
4. Inspect `isDraft`, `mergeable`, `mergeStateStatus`, `reviewDecision`, and every required check in `statusCheckRollup`.
5. Confirm selected local validation passed and the title/body follow this skill.
6. Report blockers and uncertainty. Do not claim ready while verified findings, conflicts, required checks, required reviews, or stale-head uncertainty remain.

Even when ready, merge only after explicit merge authorization. Never infer permission to comment, submit a review, close, or update the PR from a readiness request.
