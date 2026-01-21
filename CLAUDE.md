# Annie Mei

A Discord bot written in Rust that integrates with AniList, MyAnimeList, and Spotify to provide anime/manga information and theme songs.

## Quick Reference

```bash
# Build
cargo build                    # Debug build
cargo build --release          # Release build (LTO enabled)

# Run
cargo run                      # Requires env vars set

# Test & Lint
cargo test                     # Run tests
cargo clippy                   # Lint
cargo fmt                      # Format code
cargo check                    # Quick compilation check

# Database
diesel migration run           # Run pending migrations
diesel migration redo          # Redo last migration
```

## Architecture

```
src/
├── commands/           # Discord slash commands
│   ├── anime/         # /anime - fetch anime details from AniList
│   ├── manga/         # /manga - fetch manga details from AniList
│   ├── songs/         # /songs - fetch theme songs with Spotify links
│   ├── register/      # /register - link Discord user to AniList account
│   ├── help.rs        # /help - list available commands
│   └── ping.rs        # /ping - bot health check
├── models/            # Data structures
│   ├── db/           # Diesel database models
│   ├── anilist_*.rs  # AniList API response types
│   └── transformers.rs # Convert API responses to Discord embeds
├── utils/             # Utilities
│   ├── requests/     # External API clients (AniList, MAL)
│   ├── database.rs   # PostgreSQL connection
│   ├── redis.rs      # Redis caching
│   └── statics.rs    # Constants and env var names
├── schema.rs          # Diesel schema (auto-generated, don't edit)
└── main.rs            # Entry point, event handler, command routing
```

## Key Patterns

### Adding a New Command

1. Create `src/commands/newcmd/` directory with `mod.rs`, `command.rs`
2. Implement `register()` and `run()` in `command.rs`:

```rust
pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("newcmd")
        .description("What it does")
        .create_option(|option| {
            option
                .name("arg")
                .description("Argument description")
                .kind(CommandOptionType::String)
                .required(true)
        })
}

pub async fn run(ctx: &Context, interaction: &mut ApplicationCommandInteraction) {
    let _ = interaction.defer(&ctx.http).await;  // Defer immediately for long ops
    // ... command logic ...
    interaction.edit_original_interaction_response(&ctx.http, |r| r.content("Done")).await;
}
```

3. Add to `src/commands/mod.rs`
4. Register in `main.rs` `ready` event
5. Add match arm in `interaction_create`

### Blocking Operations

Use `tokio::task::spawn_blocking` for DB queries and HTTP requests:

```rust
let result = task::spawn_blocking(move || {
    // blocking code here
}).await.unwrap();
```

### Database Changes

1. Create migration: `diesel migration generate description_here`
2. Edit `up.sql` and `down.sql` in `migrations/`
3. Run: `diesel migration run`
4. Schema auto-updates in `src/schema.rs`

## Environment Variables

| Variable                 | Description                               |
| ------------------------ | ----------------------------------------- |
| `DISCORD_TOKEN`          | Discord bot token                         |
| `SENTRY_DSN`             | Sentry error tracking DSN                 |
| `ENV`                    | Environment name (development/production) |
| `DATABASE_URL`           | PostgreSQL connection string              |
| `REDIS_URL`              | Redis connection URL                      |
| `RSPOTIFY_CLIENT_ID`     | Spotify client ID                         |
| `RSPOTIFY_CLIENT_SECRET` | Spotify client secret                     |

## Code Style

- Edition 2024, rustfmt edition 2024
- Use `tracing` macros (`info!`, `debug!`, `error!`) for logging
- Add `#[instrument]` to functions for automatic tracing spans
- Prefer `?` operator over `unwrap()` where possible
- Constants go in `src/utils/statics.rs`

## Git Conventions

- **Commits**: Conventional format - `type: description` (feat, fix, docs, chore, refactor, test)
- **PR titles**: `[ANNIE-XXX]/Description` (e.g., `[ANNIE-84]/Prepare for AI Dev`)
- **Branches**: Use Linear's format - `annie-XXX-description`
- **Branch structure**: Trunk-based development with `main` as the single trunk branch

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

- Always assign PRs to `@InfernapeXavier`
- Always link to Linear issue in PR body

### Creating Releases

This project uses trunk-based development with a single `main` branch. Releases are created by tagging commits.

1. Ensure the version is bumped in `Cargo.toml` (should already be done per PR)
2. Create and push a tag:
   ```bash
   git tag vX.X.X
   git push origin vX.X.X
   ```
3. Create the GitHub release with AI-generated notes:
   ```bash
   gh release create vX.X.X --generate-notes
   ```
4. Edit release notes to include these sections:
   - **Breaking Changes** - API changes, major upgrades
   - **Improvements** - New features, enhancements
   - **Dependencies** - Package updates with version changes
