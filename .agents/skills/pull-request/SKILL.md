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
2. Read-only inspection and review may run from any checkout, including `main`. Authoring or mutating a PR must use its ticket branch; never author or mutate from `main`, and never discard unrelated changes.
3. Inspect `git status --short --branch`. For an existing PR, capture its exact current base and head before inspecting the range:

   ```bash
   gh pr view <number-or-url> --json number,url,title,body,baseRefName,baseRefOid,headRefName,headRefOid,isDraft,mergeable,mergeStateStatus,reviewDecision,statusCheckRollup
   ```

4. Fetch only when the user explicitly authorizes fetching. When authorized, fetch the captured SHAs into dedicated refs and inspect that exact range, never local `HEAD` by assumption:

   ```bash
   PR=<number>
   BASE_NAME=<baseRefName>
   BASE_SHA=<baseRefOid>
   HEAD_SHA=<headRefOid>
   BASE_REF="refs/remotes/origin/pr/$PR/base"
   HEAD_REF="refs/remotes/origin/pr/$PR/head"
   git fetch origin "$BASE_NAME:$BASE_REF" "pull/$PR/head:$HEAD_REF"
   test "$(git rev-parse "$BASE_REF")" = "$BASE_SHA"
   test "$(git rev-parse "$HEAD_REF")" = "$HEAD_SHA"
   git diff --stat "$BASE_REF...$HEAD_REF"
   git diff "$BASE_REF...$HEAD_REF"
   git log --reverse --format='%H%x09%s' "$BASE_REF..$HEAD_REF"
   ```

   If fetching is not authorized, inspect the captured PR through read-only API data (`gh pr diff <number>`, `gh api repos/{owner}/{repo}/pulls/<number>/commits`, and API file contents at `HEAD_SHA`). Refresh `headRefOid` afterward; if it changed, discard the stale inspection and repeat. State when API data cannot provide an equivalent local check.

5. For a new PR whose branch is the current checkout, establish the range against the intended base without fetching unless fetch authorization was given. Do not assume an existing local base ref is current; use API inspection or report that current-base verification requires fetch authorization.
6. Select validation according to the changed behavior, not a fixed checklist:
   - Documentation or agent guidance: `git diff --check` plus focused structural/content checks.
   - Rust changes: relevant targeted tests followed by `scripts/verify.sh` for the full sequential suite.
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
- Before opening or updating a PR, squash WIP commits only when the user explicitly authorizes that history rewrite. Never force-push without separate explicit approval.

## Review Findings

When reviewing a PR, responding to findings, or checking readiness, follow [`reference/review-threads.md`](reference/review-threads.md). It is mandatory to:

1. Retrieve all PR feedback via paginated GraphQL: inline review threads and every nested comment (including resolved and outdated threads), top-level review bodies, and general conversation comments.
2. Record `headRefOid` from every top-level feedback query and confirm all channels describe the same PR head.
3. Verify each finding against code at that exact head SHA. Never treat comment position, resolution, or age as proof that the finding still applies.
4. Re-read the PR head after retrieving and verifying all channels. If the SHA changed, discard the complete feedback snapshot and repeat retrieval and verification against the new head before reporting or mutating state.

## Readiness

Readiness is a read-only assessment, not authorization to update, review, merge, or close. Immediately before reporting readiness:

1. Confirm the assessed local or fetched commit equals the PR's current `headRefOid`.
2. Review the complete base-to-head diff and commit list.
3. Complete all-channel feedback retrieval and current-head finding verification.
4. Inspect `isDraft`, `mergeable`, `mergeStateStatus`, `reviewDecision`, and every required check in `statusCheckRollup`.
5. Confirm selected local validation passed and the title/body follow this skill.
6. Report blockers and uncertainty. Do not claim ready while verified findings, conflicts, required checks, required reviews, or stale-head uncertainty remain.

Even when ready, merge only after explicit merge authorization. Never infer permission to comment, submit a review, close, or update the PR from a readiness request.
