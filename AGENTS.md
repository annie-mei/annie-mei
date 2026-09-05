# Agent Instructions for Annie Mei

Annie Mei is a Rust Discord bot using Serenity 0.12, SQLx, Redis, Spotify, and external anime APIs. Sentry provides error reporting; the companion auth service owns OAuth schema and HTTP health endpoints.

## Permanent Git and PR Safety

- Never commit or push directly to `main`; work on a feature branch.
- Every branch must have a Linear ticket and use that ticket's suggested branch name (for example, `annie-123-description`).
- Do not create a Linear ticket automatically. Check for a suitable existing ticket, then ask before creating one.
- Never force-push without explicit approval.
- Opening, updating, commenting on, reviewing, merging, and closing a pull request are six distinct shared-state actions. Each requires explicit authorization for that action; authorization for one does not authorize any other. Read-only PR inspection does not require approval.
- PR titles must use `[ANNIE-<ticket-number>]/<description>`.
- Load and follow the project `pull-request` skill for any PR creation, update, review, comment, readiness check, merge, or close workflow.

## Code Conventions

- Run `cargo fmt`; run appropriate tests and `cargo clippy`, fixing warnings.
- Use `tracing` macros and add `#[instrument]` to public and private functions where useful, with `skip(...)`/`fields(...)` as appropriate.
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
