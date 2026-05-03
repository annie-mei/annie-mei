# OAuth data contract

This document describes the shared data contract between the
[`annie-mei`](../) Discord bot and the companion auth-service in
[`../auth`](../../auth) so future changes have an explicit reference to
check against. Any change to the field names, types, or ID
representation listed here must be coordinated in **both** repos.

The auth-service mirrors this document at
[`../auth/docs/oauth-contract.md`](../../auth/docs/oauth-contract.md).

## Overview

```diagram
╭───────────╮  /register   ╭─────────╮  callback   ╭──────────────╮
│ Discord   │─────────────▶│ Bot     │────────────▶│ AniList      │
│ user      │              │ (this   │             │ OAuth        │
╰───────────╯              │  repo)  │             ╰──────┬───────╯
                           ╰────┬────╯                    │
                                │ build_oauth_start_url   │ /oauth/anilist/callback
                                ▼                         ▼
                       ╭─────────────────╮        ╭────────────────╮
                       │ Auth-service    │◀───────│ Auth-service   │
                       │ /oauth/anilist  │        │ /oauth/anilist │
                       │   /start        │        │   /callback    │
                       ╰────────┬────────╯        ╰────────┬───────╯
                                │ writes oauth_sessions    │ writes oauth_credentials
                                ▼                          ▼
                       ╭──────────────────────────────────────────╮
                       │              Postgres (shared)           │
                       │  oauth_sessions      oauth_credentials   │
                       ╰──────────────────────────────────────────╯
                                                ▲
                                                │ reads (whoami, guild overlay)
                                                │ deletes (unregister)
                                          Annie Mei bot
```

The bot owns no Diesel-managed tables of its own. Account-link state
lives in the auth-service tables and the bot reads/deletes those rows
directly via raw SQL, sharing the same Postgres database.

## Tables

### `oauth_credentials`

Source of truth for **linked** AniList accounts.

| Column                | Type            | Notes                                                                             |
| --------------------- | --------------- | --------------------------------------------------------------------------------- |
| `discord_user_id`     | `TEXT` (PK)     | Raw Discord snowflake **as a string** (`user.id.get().to_string()`).              |
| `anilist_id`          | `BIGINT`        | AniList user ID. Unique across the table.                                         |
| `access_token`        | `TEXT`          | AniList OAuth access token.                                                       |
| `refresh_token`       | `TEXT NULL`     | AniList OAuth refresh token, when issued.                                         |
| `token_expires_at`    | `TIMESTAMPTZ`   | Token expiry, when AniList provides one.                                          |
| `token_updated_at`    | `TIMESTAMPTZ`   | Last token write time.                                                            |
| `created_at`          | `TIMESTAMPTZ`   | Initial link time.                                                                |
| `relink_required_at`  | `TIMESTAMPTZ`   | Set when the auth-service decides the user must re-run `/register`.               |
| `relink_reason`       | `TEXT NULL`     | Free-text reason that pairs with `relink_required_at`.                            |

**Bot reads:** [`crate::models::db::oauth_credential::OAuthCredential`](../src/models/db/oauth_credential.rs)
provides `get_by_discord_id` (used by `/whoami`) and
`get_by_discord_ids` (used by the per-guild MediaList overlay in
`crate::utils::guild`). The bot does **not** write to this table.

**Bot deletes:** `/unregister` deletes by `discord_user_id` in the
same transaction as `oauth_sessions`. See
[`src/commands/unregister.rs`](../src/commands/unregister.rs).

### `oauth_sessions`

Short-lived state for in-flight OAuth flows.

| Column            | Type           | Notes                                                                       |
| ----------------- | -------------- | --------------------------------------------------------------------------- |
| `state`           | `TEXT` (PK)    | Opaque per-flow state token issued by the auth-service.                     |
| `discord_user_id` | `TEXT`         | Raw Discord snowflake string, mirroring `oauth_credentials.discord_user_id`. |
| `expires_at`      | `TIMESTAMPTZ`  | When the session token stops being valid.                                   |
| `used_at`         | `TIMESTAMPTZ`  | When the session was redeemed (replay protection).                          |
| `created_at`      | `TIMESTAMPTZ`  | Initial creation time.                                                      |

**Bot deletes:** `/unregister` includes a `DELETE FROM oauth_sessions
WHERE discord_user_id = $1` in the same transaction so half-finished
flows do not linger after a user unlinks.

## OAuth context payload

`/register` builds an opaque `ctx` query parameter that the
auth-service consumes at `/oauth/anilist/start`. It is a
base64-url-encoded JSON payload signed with HMAC-SHA256 using the
shared `OAUTH_CONTEXT_SIGNING_SECRET`. See
[`crate::utils::oauth`](../src/utils/oauth/mod.rs).

| Field             | Type     | Notes                                                                                                          |
| ----------------- | -------- | -------------------------------------------------------------------------------------------------------------- |
| `v`               | `u8`     | Schema version. Currently `1`. Bump in both repos when fields change.                                          |
| `discord_user_id` | `string` | Raw Discord snowflake string.                                                                                  |
| `guild_id`        | `string` | Optional. Set when the `/register` interaction came from a guild.                                              |
| `interaction_id`  | `string` | Discord interaction ID for traceability.                                                                       |
| `nonce`           | `string` | 16-byte URL-safe base64 nonce.                                                                                 |
| `iat`             | `i64`    | Issued-at unix seconds.                                                                                        |
| `exp`             | `i64`    | Expiry unix seconds. Default TTL is `OAUTH_CONTEXT_TTL_SECONDS` (300s).                                        |

The signature segment is appended after `.` to form
`<payload_b64>.<sig_b64>`, identical on both sides.

## Coordinated change checklist

Make the matching change in both repos in the same release window if
you touch any of the following:

- A column on `oauth_credentials` or `oauth_sessions` (rename, type
  change, deletion, or new NOT NULL column).
- The `discord_user_id` representation (it must stay the raw Discord
  snowflake stringified via `user.id.get().to_string()` so bot
  cleanup queries continue to match auth writes).
- Any field on the OAuth context payload, or the `v` version constant.
- The `OAUTH_CONTEXT_SIGNING_SECRET` value or signing algorithm.

When making one of those changes:

1. Land the schema migration in the auth repo first.
2. Update the bot model / SQL in this repo.
3. Update both copies of `docs/oauth-contract.md`.
4. Coordinate the deploy so the auth-service migration runs before
   bot binaries that depend on the new shape are released.

## Privacy

`oauth_credentials.discord_user_id` and `oauth_sessions.discord_user_id`
intentionally store the raw Discord snowflake so that `/register`,
`/whoami`, and `/unregister` can all match on the same value. **Logs,
spans, and Sentry telemetry must not include the raw snowflake.** Use
`crate::utils::privacy::hash_user_id` (bot) or
`utils::observability::identifier_fingerprint` (auth-service) to emit
a salted hash instead. Both helpers use the shared `USERID_HASH_SALT`
environment variable so fingerprints correlate across repos.
