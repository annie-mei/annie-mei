# Agent Instructions for Annie Mei

This document provides context for AI coding agents working on the Annie Mei Discord bot.

## Project Summary

Annie Mei is a Rust Discord bot using Serenity 0.12 that fetches anime/manga data from AniList and theme songs from MyAnimeList/Spotify. Users interact via Discord slash commands. The app also exposes a small Axum health server and reports errors/logs to Sentry.

## Technology Stack

| Component | Technology                   |
| --------- | ---------------------------- |
| Language  | Rust 2024 Edition            |
| Discord   | Serenity 0.12                |
| Database  | PostgreSQL + Diesel ORM      |
| Cache     | Redis                        |
| HTTP      | Reqwest (blocking) + Axum    |
| Async     | Tokio                        |
| Logging   | tracing + tracing-subscriber |
| Errors    | Sentry                       |

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

## Essential Commands

```bash
cargo build              # Compile debug build
cargo build --release    # Compile optimized build
cargo test               # Run test suite
cargo clippy             # Run linter
cargo fmt                # Format code
cargo check              # Fast type checking
diesel migration run     # Apply database migrations
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

- Use conventional commit format: `type: description`
- Types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`
- Make small, sensible commits as you go; avoid batching unrelated changes into one commit

### Git Safety

- **NEVER commit or push directly to `main`** - Always create a feature branch first
- **All branches must have a Linear ticket** - Use the ticket's suggested branch name (e.g., `annie-XXX-description`). Only create a ticketless branch if the user explicitly approves it
- **Don't create tickets automatically** - Check for existing/recent tickets that the work might fit under, and ask the user before creating a new one
- **Never force push** - Always ask before any destructive git operation
- **When git issues occur** (failed push, wrong commit, merge conflicts, etc.):
  1. Explain what went wrong
  2. Present the available options
  3. Ask the user how they want to resolve it
- If a push didn't go through, prefer `git reset --hard origin/<branch>` over amending and force pushing

### Versioning

Bump the version in `Cargo.toml` with every PR using semantic versioning:

- **MAJOR** (X.0.0): Breaking changes, incompatible API changes
- **MINOR** (0.X.0): New features, backwards-compatible functionality
- **PATCH** (0.0.X): Bug fixes, backwards-compatible patches

Examples:

- New command or feature → bump minor
- Bug fix or refactor → bump patch
- Breaking change to existing behavior → bump major

- Also commit the Cargo.lock file when bumping the version — run `cargo check` to update the lockfile

### Pull Requests

- Title format: `[ANNIE-XXX]/Description`
- Example: `[ANNIE-84]/Prepare for AI Dev`
- Always link to Linear issue in PR body
- Always assign the PR to `@InfernapeXavier`

### Creating Releases

This project uses trunk-based development with a single `main` branch. Releases are created by tagging commits.

1. Ensure the version is bumped in `Cargo.toml` (should already be done per PR)
2. Create and push a tag:
   ```bash
   git tag vX.X.X
   git push origin vX.X.X
   ```
3. Create the GitHub release with generated notes:
   ```bash
   gh release create vX.X.X --generate-notes
   ```
4. The `build-release.yml` workflow will attach binaries and deploy automatically
5. Edit release notes to include these sections:
   - **Breaking Changes** - API changes, major upgrades
   - **Improvements** - New features, enhancements
   - **Dependencies** - Package updates with version changes

### Branches

- Use Linear's suggested branch name: `annie-XXX-description`
- `main` - Single trunk branch (all PRs target this)

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

### Async/Blocking Patterns

- Discord interactions are async
- External API calls use blocking reqwest
- Database access and Redis access are also synchronous today
- Wrap blocking HTTP/DB/Redis work in `tokio::task::spawn_blocking`
- Always `defer()` interactions before long operations

### Observability and Privacy

- Use `tracing` spans with explicit names for command entrypoints and helpers
- Preserve Sentry integration and hashed Discord user IDs when touching observability-related code
- Never log raw secrets or credential-bearing URLs; use existing privacy helpers in `src/utils/privacy.rs`
- `main.rs` supports a CLI helper command: `cargo run -- hash <discord_user_id>` for Sentry correlation

## File Patterns

| Pattern            | Location                         |
| ------------------ | -------------------------------- |
| Slash commands     | `src/commands/<name>/command.rs` or `src/commands/<name>.rs` |
| API response types | `src/models/anilist_*.rs`        |
| Database models    | `src/models/db/*.rs`             |
| API clients        | `src/utils/requests/*.rs`        |
| Constants          | `src/utils/statics.rs`           |
| GraphQL queries    | `src/commands/<name>/queries.rs` |
| Health server      | `src/server.rs`                  |

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
