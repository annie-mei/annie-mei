# Agent Instructions for Annie Mei

<!-- BEGIN ANNIE MEI SHARED WORKFLOW -->
<!-- Source: annie-mei/.github/AGENTS.md. Keep this block identical in all four repos. -->
## Annie Mei organization workflow

Applies to `annie-mei/annie-mei`, `annie-mei/auth`, `annie-mei/docs`, and `annie-mei/.github`.

- Every work branch must have a Linear ticket and use the ticket's suggested branch name.
- Check existing and recent tickets before proposing a new one. Ask the user before creating a ticket.
- PR titles must use `[ANNIE-<ticket-number>]/<description>`.
- Do not mention or link Linear tickets in PR descriptions; branch automation supplies the link.
- Fill the repository's existing `.github/PULL_REQUEST_TEMPLATE.md`. Do not duplicate the template in agent guidance.
- If the repository has no PR template, reuse the existing bot template: https://github.com/annie-mei/annie-mei/blob/main/.github/PULL_REQUEST_TEMPLATE.md
- Remove placeholder comments and unused optional sections. Record validation actually performed and replace `MODEL_NAME` with the actual model.
<!-- END ANNIE MEI SHARED WORKFLOW -->

Annie Mei is a Rust Discord bot using Serenity 0.12, SQLx, Redis, Spotify, and external anime APIs. Sentry provides error reporting; the companion auth service owns OAuth schema and HTTP health endpoints.

## Permanent Git and PR Safety

- Never commit or push directly to `main`; work on a feature branch.
- Never force-push without explicit approval.
- Opening, updating, commenting on, reviewing, merging, and closing a pull request are six distinct shared-state actions. Each requires explicit authorization for that action; authorization for one does not authorize any other. Read-only PR inspection does not require approval.
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
