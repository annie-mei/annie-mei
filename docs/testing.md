# Testing Discord command plumbing

Annie Mei uses two complementary test layers: deterministic protocol tests
that run without Discord credentials, and an opt-in live smoke test in a
dedicated Discord guild.

## Automated protocol tests

Run the Discord plumbing suite with:

```sh
cargo test --test discord_plumbing
```

The suite deserializes synthetic Discord interaction payloads into Serenity
models, resolves each command through Annie Mei's command catalog, and points
Serenity's real HTTP client at a local mock server. It currently verifies:

- `/ping` sends the expected immediate interaction callback route and JSON;
- invalid `/anime` input sends a deferred callback followed by an edit to the
  original interaction response; and
- command definitions have unique names that match the runtime router.

Fixtures under `tests/fixtures/interactions/` contain only synthetic IDs and
tokens. These tests must remain deterministic and must not require Discord,
AniList, MAL, Spotify, LLM, database, or Redis credentials.

The complete repository suite remains:

```sh
cargo test --all-features
```

## Live Discord smoke test

Use this only when a change needs confirmation across Discord's real Gateway,
command registration, permissions, and client rendering.

### Prerequisites

1. Create a Discord application used only for testing and add its bot to a
   private test guild with the `bot` and `applications.commands` scopes.
2. Use test-specific infrastructure and credentials. Never point the smoke
   run at the production bot token, guild, database, or Redis instance.
3. Configure the environment variables required by the bot, using the test
   application's token as `DISCORD_TOKEN`.

### Checklist

1. Start Annie Mei with `cargo run` and wait for the ready log confirming that
   commands were registered.
2. In the Discord client, invoke `/ping`.
   - Confirm Discord shows a single immediate response mentioning the caller.
   - Confirm the bot logs receipt of the command without exposing a raw user ID
     or interaction token.
3. Invoke `/anime search:One Piece`.
   - Confirm Discord shows the deferred/loading state before the final embed.
   - Confirm the final response replaces the loading state rather than posting
     an unrelated second message.
4. Invoke `/anime` with a search term longer than 255 characters.
   - Confirm the deferred response is replaced with the validation message and
     no AniList result is shown.
5. Invoke `/settings` and use one displayed control.
   - Confirm the component interaction updates or acknowledges the original
     response without Discord reporting that the interaction failed.
6. Stop the bot and retain only redacted logs needed to diagnose failures.

The command invocations are intentionally manual. Discord does not expose a
supported API for invoking a slash command as a user, bot accounts cannot do
so on a user's behalf, and automating a normal user account is prohibited
self-botting. An agent can prepare the environment, start the bot, and inspect
the results, but a human must perform the Discord-client actions.
