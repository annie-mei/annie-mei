# Agent Instructions for Annie Mei

This document provides context for AI coding agents working on the Annie Mei Discord bot.

## Project Summary

Annie Mei is a Rust Discord bot using Serenity 0.11 that fetches anime/manga data from AniList and theme songs from MyAnimeList/Spotify. Users interact via Discord slash commands.

## Technology Stack

| Component | Technology                   |
| --------- | ---------------------------- |
| Language  | Rust 2024 Edition            |
| Discord   | Serenity 0.12                |
| Database  | PostgreSQL + Diesel ORM      |
| Cache     | Redis                        |
| HTTP      | Reqwest (blocking)           |
| Async     | Tokio                        |
| Logging   | tracing + tracing-subscriber |
| Errors    | Sentry                       |

## Project Layout

```
src/
├── commands/        # Slash command implementations
├── models/          # Data types, DB models, API responses
├── utils/           # Shared utilities, API clients, DB helpers
├── schema.rs        # Diesel schema (AUTO-GENERATED - never edit)
└── main.rs          # Bot entry point and event routing
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
- Prefer `?` operator over `.unwrap()` for error handling

### Git Commits

- Use conventional commit format: `type: description`
- Types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`

### Git Safety

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
2. Implement `register()` for slash command definition
3. Implement `run()` for command execution
4. Export in `src/commands/mod.rs`
5. Register in `main.rs` `ready` event
6. Add match arm in `main.rs` `interaction_create`

### Database Changes

1. Generate migration: `diesel migration generate name`
2. Write SQL in `migrations/*/up.sql` and `down.sql`
3. Run: `diesel migration run`
4. `src/schema.rs` updates automatically

### Async/Blocking Patterns

- Discord interactions are async
- External API calls use blocking reqwest
- Wrap blocking code in `tokio::task::spawn_blocking`
- Always `defer()` interactions before long operations

## File Patterns

| Pattern            | Location                         |
| ------------------ | -------------------------------- |
| Slash commands     | `src/commands/<name>/command.rs` |
| API response types | `src/models/anilist_*.rs`        |
| Database models    | `src/models/db/*.rs`             |
| API clients        | `src/utils/requests/*.rs`        |
| Constants          | `src/utils/statics.rs`           |
| GraphQL queries    | `src/commands/<name>/queries.rs` |

## Testing

- Unit tests go in the same file as the code being tested
- Integration tests go in `tests/` directory
- Mock external APIs in tests
- Run `cargo test` to execute all tests

## Common Pitfalls

1. **Don't edit `src/schema.rs`** - It's auto-generated by Diesel
2. **Always defer long operations** - Discord has a 3-second response window
3. **Use spawn_blocking for HTTP/DB** - Reqwest blocking client blocks the thread
4. **Check environment variables** - Bot requires multiple env vars to run

## Environment Requirements

Required environment variables:

- `DISCORD_TOKEN` - Bot token from Discord Developer Portal
- `SENTRY_DSN` - Sentry project DSN
- `ENV` - Environment name
- `DATABASE_URL` - PostgreSQL connection string
- `REDIS_URL` - Redis connection string
- `RSPOTIFY_CLIENT_ID` - Spotify API client ID
- `RSPOTIFY_CLIENT_SECRET` - Spotify API client secret
