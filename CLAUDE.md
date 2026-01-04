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
- **Branch structure**: `next` (development), `current` (production/release)

### Pull Requests

- Always assign PRs to `@InfernapeXavier`
- Always link to Linear issue in PR body

### Release Process

1. Create PR from `next` to `current`:
   - Title: `[Annie Mei]/Release X.X.X`
   - Add the `release` label
   - Assign to `@InfernapeXavier`

2. After merge, create release with AI-generated notes:
   ```bash
   gh release create vX.X.X --target current --notes "AI-generated release notes"
   ```

3. Release notes sections:
   - **Breaking Changes** - API changes, major upgrades
   - **Improvements** - New features, enhancements
   - **Dependencies** - Package updates with version changes
