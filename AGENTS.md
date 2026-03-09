# Agent Instructions for Annie Mei

This document provides context for AI coding agents working on the Annie Mei Discord bot.

## Project Summary

Annie Mei is a Rust Discord bot using Serenity 0.12 that fetches anime/manga data from AniList and theme songs from MyAnimeList/Spotify. Users interact via Discord slash commands. The app also exposes a small Axum health server and reports errors/logs to Sentry.

## Project Layout

```
src/
├── commands/        # Slash command implementations
├── models/          # Data types, DB models, API responses
├── server.rs        # Axum health server (/healthz)
├── utils/           # Shared utilities, API clients, DB helpers
├── schema.rs        # Diesel schema (AUTO-GENERATED - never edit)
└── main.rs          # Bot entry point, event routing, startup/shutdown
migrations/          # Diesel SQL migrations
```

## Conventions to Follow

### Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy` and fix warnings
- Use `tracing` macros for logging (`info!`, `debug!`, `error!`)
- Add `#[instrument]` attribute to functions for tracing spans
- Add `#[instrument]` to private/helper functions too (for example query builders and alias helpers), keeping signatures unchanged and using `skip(...)`/`fields(...)` when useful
- When implementing review findings, first verify the current code state and only apply changes that are actually missing
- Prefer `?` operator over `.unwrap()` for error handling
- Preserve the newer "core handler + thin Serenity adapter" pattern when extending commands; prefer returning `CommandResponse` from testable logic and keeping Discord transport concerns in `run()`
- Keep embed construction in shared model/transformer code when possible instead of building large embeds inline in command handlers

### Git Commits

- Use Conventional Commits and prefer `type(scope): summary`
- Example: `feat(anime): add guild score fallback`
- Types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`
- Make small, sensible commits as you go; avoid batching unrelated changes into one commit
- Squash WIP commits before opening a PR

### Git Safety

- **NEVER commit or push directly to `main`** - Always create a feature branch first
- **All branches must have a Linear ticket** - Use the ticket's suggested branch name (e.g., `annie-XXX-description`)
- **Don't create tickets automatically** - Check for existing/recent tickets that the work might fit under, and ask the user before creating a new one
- **Never force push** - Always ask before any destructive git operation
- **When git issues occur** (failed push, wrong commit, merge conflicts, etc.):
  1. Explain what went wrong
  2. Present the available options
  3. Ask the user how they want to resolve it

### Versioning

Bump the version in `Cargo.toml` using semantic versioning when preparing versioned changes:

- **MAJOR** (X.0.0): Breaking changes, incompatible API changes
- **MINOR** (0.X.0): New features, backwards-compatible functionality
- **PATCH** (0.0.X): Bug fixes, backwards-compatible patches

Examples:

- New command or feature → bump minor
- Bug fix or refactor → bump patch
- Breaking change to existing behavior → bump major

- Also commit the Cargo.lock file when bumping the version — run `cargo check` to update the lockfile

### Pull Requests

- PR titles should use `[ANNIE-<ticket-number>]/<description>`
- PR descriptions should include:
  - `## Summary` describing the change at a high level
  - `## Type of Change` with relevant checklist items
  - `## Changes` with bullets in `full-commit-sha: description` format (no code formatting)
  - `### Notes` under Changes when implementation details matter
  - `### High-risk resources` under Changes when applicable
  - `## Validation` with appropriate test and QA steps for the scope of the change
  - `## References` for relevant dashboards, docs, issues, or runbooks when useful
  - A closing footnote replacing `MODEL_NAME` with the actual model used to write the PR

PR template:

```md
## Summary

## Type of Change

- [ ] Bug fix
- [ ] New feature
- [ ] Refactor
- [ ] Documentation
- [ ] Chore

## Changes
- <full-commit-sha>: <what changed>

### Notes (optional)

### High-risk resources (optional)

## Validation
- [ ] <relevant cargo test / cargo clippy / manual verification steps>
- [ ] <Discord QA or runtime verification, if applicable>

## References (optional)

---

This PR description was written by MODEL_NAME.
```

### Adding Commands

1. Create module in `src/commands/`
2. If the command is substantial, prefer a `src/commands/<name>/command.rs` module with a transport-agnostic core handler and a thin `run()` adapter
3. Implement `register()` for slash command definition
4. Implement `run()` for command execution
5. Export in `src/commands/mod.rs`
6. Register in `main.rs` `ready` event
7. Add match arm in `main.rs` `interaction_create`

Notes:

- Not every command uses a folder yet; smaller legacy commands like `src/commands/help.rs` and `src/commands/ping.rs` are still flat files
- Reuse `src/commands/response.rs` and `src/commands/traits.rs` patterns where practical so logic stays unit-testable without Discord runtime dependencies

### Database Changes

1. Generate migration: `diesel migration generate name`
2. Write SQL in `migrations/*/up.sql` and `down.sql`
3. Run: `diesel migration run`
4. `src/schema.rs` updates automatically

## Testing

- Unit tests go in the same file as the code being tested
- Integration tests go in `tests/` directory
- Mock external APIs in tests
- Run `cargo test` to execute all tests

## Common Pitfalls

1. **Don't edit `src/schema.rs`** - It's auto-generated by Diesel
2. **Always defer long operations** - Discord has a 3-second response window
3. **Use spawn_blocking for HTTP/DB/Redis** - external I/O is largely synchronous under the hood
4. **Global command registration is slow to propagate** - `main.rs` re-registers global slash commands on startup
5. **Check environment variables** - Bot requires multiple env vars to run
6. **Don't assume all commands follow the same file shape** - some are folder-based, others are single files

## Review Guidelines

- Confirm command changes follow the existing registration and dispatch structure in `src/main.rs`
- Prefer the core-handler plus thin-Serenity-adapter pattern for substantial command work
- Verify blocking HTTP/DB/Redis work uses `tokio::task::spawn_blocking`
- Ensure long-running Discord interactions defer before doing expensive work
- Check that `tracing` and `#[instrument]` are added where they provide useful observability
- Verify secrets, credential-bearing URLs, and raw Discord user IDs are not exposed in logs or code
- Ensure `src/schema.rs` is not edited manually; schema changes should come from Diesel migrations
- Confirm validation matches the scope: targeted tests, `cargo test`, `cargo clippy`, and manual Discord or `/healthz` verification when applicable
- For versioned changes, verify the `Cargo.toml` bump is correct and `Cargo.lock` is updated

## Key Paths

- `src/main.rs` - bot startup, command registration/dispatch, shutdown, Sentry setup
- `src/server.rs` - `/healthz` server
- `src/commands/<name>/command.rs` or `src/commands/<name>.rs` - slash commands
- `src/commands/response.rs` and `src/commands/traits.rs` - testable command patterns
- `src/models/transformers.rs` - shared embed construction
- `src/utils/requests/*.rs` - upstream API clients
- `src/utils/privacy.rs` - hashed IDs and URL redaction

## Environment Requirements

Required environment variables:

- `DISCORD_TOKEN` - Bot token from Discord Developer Portal
- `SENTRY_DSN` - Sentry project DSN
- `ENV` - Environment name
- `SENTRY_TRACES_SAMPLE_RATE` - Sentry tracing sample rate from 0.0 to 1.0
- `DATABASE_URL` - PostgreSQL connection string
- `REDIS_URL` - Redis connection string
- `SPOTIFY_CLIENT_ID` - Spotify API client ID
- `SPOTIFY_CLIENT_SECRET` - Spotify API client secret
- `MAL_CLIENT_ID` - MyAnimeList API client ID
- `USERID_HASH_SALT` - Salt used when hashing Discord user IDs for Sentry/log correlation
- `SERVER_PORT` - Optional local HTTP server port (defaults to 8080)
