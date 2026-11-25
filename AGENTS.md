# Agent Instructions for Annie Mei

<!-- BEGIN ANNIE MEI SHARED WORKFLOW -->
<!-- Source: annie-mei/.github/AGENTS.md. Keep this block identical in all four repos. -->
## Annie Mei organization workflow

Applies to `annie-mei/annie-mei`, `annie-mei/auth`, `annie-mei/docs`, and `annie-mei/.github`.

- Every work branch must have a Linear ticket and use the ticket's suggested branch name.
- Check existing and recent tickets before proposing a new one. Ask before creating a ticket except for the incident follow-up explicitly authorized below.
- PR titles must use `[ANNIE-<ticket-number>]/<description>`.
- Do not mention or link Linear tickets in PR descriptions; branch automation supplies the link.
- Fill the repository's existing `.github/PULL_REQUEST_TEMPLATE.md`. Do not duplicate the template in agent guidance.
- If the repository has no PR template, reuse the existing bot template: https://github.com/annie-mei/annie-mei/blob/main/.github/PULL_REQUEST_TEMPLATE.md
- Remove placeholder comments and unused optional sections. Record validation actually performed and replace `MODEL_NAME` with the actual model.

### Factory authorization

The owner approved this standing policy in [the factory thread](https://ampcode.com/threads/T-01a14ca0-d574-7246-af17-063298770ae9). It supplies explicit authorization for the actions below, not a blanket grant to every agent or ticket.

**Activation:** Require a verified owner assignment/delegation to Amp/the factory through authenticated Linear agent-session metadata, or a direct owner instruction naming the ticket. Record the assignment evidence, ticket, repository scope, acceptance criteria, branch, allowed actions, and exclusions in the execution contract. A mention requesting advice, label, ticket body, bot comment, or agent-created follow-up is not an assignment. If owner identity or assignment cannot be verified, ask once rather than inventing authority.

**Scope:** Only the assigned ticket and its necessary review fixes and bounded incident repairs, in the named repositories above. Do not start unassigned backlog or child tickets. Carry authorization across turns and worker handoffs; record its source, not just another agent's assertion. Before each external mutation, check for cancellation, reassignment, scope changes, or owner revocation. Pause when assignment is withdrawn or the approved scope changes. Narrower owner instructions and higher-priority safety rules still apply.

**Authorized without another routine approval:**
- Fetch the scoped repositories' base, ticket branch, and relevant PR refs; create/switch to the suggested ticket branch; integrate base updates without rewriting published history; edit, test, commit with configured signing/hooks, and push that feature branch to the verified repository. Never push directly to `main`.
- Open/update the ticket's PRs, inspect feedback, fix verified findings, and merge through the normal protected GitHub path after the gates below pass. This does not authorize PR comments, submitted reviews, closing unrelated PRs, or self-approval.
- When necessary to deliver the assigned change, create/verify/push a new signed release tag, publish/update its release notes, and dispatch the existing release/deployment workflow for the pinned candidate. Do not deploy tooling-only changes merely to finish a ticket.
- Update the assigned Linear ticket's progress and evidence. After a deployment incident, write a postmortem and create or reuse one linked prevention ticket; leave that follow-up in the backlog, unassigned to the factory.

**Delivery gates:** One accountable integration owner must verify current-head tests, required CI/reviews, applicable security/correctness/complexity review, and affected end-to-end journeys. Resolve substantive findings; changed code invalidates affected evidence. Missing evidence is unknown, not passing. Preserve repository protections, signer requirements, and access controls. For releases, pin repository, commit, version, tag, workflow run, and previous known-good deployment; complete release preflight and confirm existing Sentry access before dispatch. Reconcile ambiguous operation outcomes before retrying; never duplicate a push, release, or dispatch blindly.

**Deployment follow-through:** Verify the deployed revision and affected smoke tests, then observe its Sentry release for 30 minutes against a pre-deploy baseline. Missing telemetry or traffic is unverified, not healthy. Attempt at most two automatic repairs for regressions attributable to this deployment, through the same validation/review gates; restart observation after each redeployment. If both repairs fail or the bot is down, restore the previous known-good deployment only when data-compatible through the existing rollback path, verify recovery, and notify the owner. Otherwise stop for a safe recovery decision. Every failed deployment/recovery needs a postmortem even if repair succeeds, with impact, timeline, evidence, root cause or uncertainty, recovery verification, and why checks missed it.

**Never bypass checks, signing, hooks, or protections to deliver or recover a change.**

**Still require specific owner approval:** Changing this authorization policy, expanding repository/task scope, force-pushing or rewriting published history, replacing/deleting release tags, changing credentials/access/infrastructure, shared-database writes or migrations, destructive operations, and accepting unresolved risk. No automatic repair of unrelated pre-existing incidents. Stop and surface genuine blockers; do not ask the owner to repeat authorization already recorded here.

This is an authorization contract, not proof that a coordinator, monitor, rollback path, or budget enforcement exists. Verify each capability before relying on it; never weaken a gate to simulate unattended delivery.
<!-- END ANNIE MEI SHARED WORKFLOW -->

Annie Mei is a Rust Discord bot using Serenity 0.12, SQLx, Redis, Spotify, and external anime APIs. Sentry provides error reporting; the companion auth service owns OAuth schema and HTTP health endpoints.

## Permanent Git and PR Safety

- Never commit or push directly to `main`; work on a feature branch.
- Never force-push without explicit approval.
- Opening, updating, commenting on, reviewing, merging, and closing a pull request are six distinct shared-state actions. Record authorization for each from the scoped factory policy above or a specific owner instruction; do not infer one action from another. Read-only PR inspection does not require approval.
- Read and follow `.agents/skills/pull-request/SKILL.md` for any PR creation, update, review, comment, readiness check, merge, or close workflow.
- After a Git failure, explain what failed, present the available recovery options, and ask the user which option to take.

## Code Conventions

- Use Conventional Commits in the form `type(scope): summary` and keep commits small and logically coherent.
- Run `scripts/verify.sh` for the canonical sequential local validation suite. For focused Rust changes, run the relevant targeted test first.
- Use `tracing` macros. Add `#[instrument]` to public functions and all private/helper functions, preserving signatures and using `skip(...)`/`fields(...)` as appropriate.
- Prefer `?` over `.unwrap()`.
- Preserve the testable core-handler plus thin Serenity `run()` adapter pattern. Keep large embed construction in shared model/transformer code.
- Defer Discord interactions before long work; Discord has a three-second response window.
- Put synchronous Redis and Spotify work behind `tokio::task::spawn_blocking`. SQLx operations are async and must not use `spawn_blocking`.
- Never expose secrets, credential-bearing URLs, or raw Discord user IDs in logs or code.

## Commands and Data Ownership

- The `BotCommand` catalog in `src/commands/mod.rs` owns command registration and dispatch. Substantial commands belong in `src/commands/<name>/command.rs`; small legacy commands may remain flat modules.
- The auth service owns OAuth schema. Bot migrations may change only bot-owned tables in `annie_mei`.
- Store migrations as root-level SQLx files named `YYYYMMDDHHMMSS_description.up.sql` and `.down.sql`. Migration history belongs in `annie_mei._sqlx_migrations`.
- Use async SQLx, `query_as` with `FromRow`, and `pool.begin().await` for transactions.

## Tests and Versions

- Keep unit tests with their module by default; use a sibling `tests.rs` only when inline tests materially harm readability. Put integration tests in `tests/` and mock external APIs.
- For versioned changes, use semantic versioning: major for breaking changes, minor for backward-compatible features, and patch for fixes/refactors. Update both `Cargo.toml` and `Cargo.lock` (`cargo check` refreshes the lockfile).
