# OAuth data contract

This document describes the shared data contract between the
[`annie-mei`](../) Discord bot and the companion auth-service in
[`auth`](https://github.com/annie-mei/auth) so future changes have an
explicit reference to check against. Any change to the field names,
types, or ID representation listed here must be coordinated in
**both** repos.

The auth-service mirrors this document at
[`auth/docs/oauth-contract.md`](https://github.com/annie-mei/auth/blob/main/docs/oauth-contract.md).

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
                                 │ writes annie_auth.oauth_sessions │ writes annie_auth.oauth_credentials
                                ▼                          ▼
                       ╭──────────────────────────────────────────╮
                        │              Postgres (shared)           │
                        │ annie_auth.oauth_sessions  annie_auth.oauth_credentials │
                       ╰──────────────────────────────────────────╯
                                                ▲
                                                │ reads (whoami, guild overlay)
                                                │ deletes (unregister)
                                          Annie Mei bot
```

Account-link state lives in the auth-service-owned `annie_auth` schema, and
the bot reads/deletes those rows directly via SQLx. Bot-owned settings
tables live in the `annie_mei` schema. Schema ownership and migration
isolation are documented in [Database schema ownership](database-schemas.md).

## Tables

### `annie_auth.oauth_credentials`

Source of truth for **linked** AniList accounts.

| Column                | Type            | Notes                                                                             |
| --------------------- | --------------- | --------------------------------------------------------------------------------- |
| `discord_user_id`     | `TEXT` (PK)     | Raw Discord snowflake **as a string** (`user.id.get().to_string()`).              |
| `anilist_id`          | `BIGINT`        | AniList user ID. Unique across the table.                                         |
| `anilist_username`    | `TEXT NULL`     | Public AniList username captured from the OAuth viewer response for friendly linked-account displays. |
| `access_token`        | `TEXT`          | AniList OAuth access token.                                                       |
| `refresh_token`       | `TEXT NULL`     | AniList OAuth refresh token, when issued.                                         |
| `token_expires_at`    | `TIMESTAMPTZ NULL` | Token expiry, when AniList provides one.                                          |
| `token_updated_at`    | `TIMESTAMPTZ`   | Last token write time.                                                            |
| `created_at`          | `TIMESTAMPTZ`   | Initial link time.                                                                |
| `relink_required_at`  | `TIMESTAMPTZ NULL` | Set when the auth-service decides the user must re-run `/register`.               |
| `relink_reason`       | `TEXT NULL`     | Free-text reason that pairs with `relink_required_at`.                            |

**Bot reads:** [`crate::models::db::oauth_credential::OAuthCredential`](../src/models/db/oauth_credential.rs)
provides `get_by_discord_id` (used by `/whoami`) and
`get_by_discord_ids` (used by the per-guild MediaList overlay in
`crate::utils::guild`). The bot does **not** write to this table.

`anilist_username` is nullable so existing linked users can keep working
after the auth-service migration. It is populated by the auth-service on
the next successful OAuth callback/relink. Existing rows can be
backfilled by querying AniList for each stored `anilist_id` and updating
`anilist_username`, or users can run `/register` again after
`/unregister` to relink. Until then, bot displays fall back to the
numeric `anilist_id`.

**Bot deletes:** `/unregister` deletes by `discord_user_id` in the
same transaction as `oauth_sessions`. See
[`src/commands/unregister.rs`](../src/commands/unregister.rs).

### `annie_auth.oauth_sessions`

Short-lived state for in-flight OAuth flows.

| Column            | Type           | Notes                                                                       |
| ----------------- | -------------- | --------------------------------------------------------------------------- |
| `state`           | `TEXT` (PK)    | Opaque per-flow state token issued by the auth-service.                     |
| `discord_user_id` | `TEXT`         | Raw Discord snowflake string, mirroring `annie_auth.oauth_credentials.discord_user_id`. |
| `expires_at`      | `TIMESTAMPTZ`  | When the session token stops being valid.                                   |
| `used_at`         | `TIMESTAMPTZ NULL` | When the session was redeemed (replay protection).                          |
| `created_at`      | `TIMESTAMPTZ`  | Initial creation time.                                                      |

**Bot deletes:** `/unregister` includes a `DELETE FROM annie_auth.oauth_sessions
WHERE discord_user_id = $1` in the same transaction so half-finished
flows do not linger after a user unlinks.

### `annie_mei.user_settings` and `annie_mei.guild_settings`

The bot owns configurable user and guild settings in the `annie_mei`
schema. Auth-service migrations must not create, alter, or drop these
tables.

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

- A column on `annie_auth.oauth_credentials` or `annie_auth.oauth_sessions` (rename, type
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

`annie_auth.oauth_credentials.discord_user_id` and `annie_auth.oauth_sessions.discord_user_id`
intentionally store the raw Discord snowflake so that `/register`,
`/whoami`, and `/unregister` can all match on the same value. The
`annie_auth.oauth_credentials.anilist_id` and `annie_auth.oauth_credentials.anilist_username`
fields are also user-identifying account-linkage data. **Logs, spans,
metrics labels, breadcrumbs, and Sentry telemetry must not include any
of those raw identifiers.** Use `crate::utils::privacy::hash_user_id`
(bot) or `utils::observability::identifier_fingerprint` (auth-service)
to emit a salted hash when a stable correlation key is needed. Both
helpers use the shared `USERID_HASH_SALT` environment variable so
fingerprints correlate across repos.

The `ctx` query parameter on `/oauth/anilist/start` is base64url-encoded
JSON plus a signature, not encrypted data. It contains raw
`discord_user_id`, `guild_id`, and `interaction_id` values, so request
URL capture can leak those identifiers even when application logs use
fingerprints. Strip or redact the full `ctx` value from HTTP access logs,
Sentry transactions, request breadcrumbs, distributed tracing spans, and
any reverse-proxy logs before those observability records leave the
service.

`annie_auth.oauth_credentials.access_token` and `annie_auth.oauth_credentials.refresh_token`
are bearer credentials that grant full access to the linked AniList
account. The bot's current `OAuthCredential` model intentionally only
selects `discord_user_id`, `anilist_id`, and `anilist_username`, but if
a future change expands the projection to read either token column,
**the values must never appear in logs, spans, breadcrumbs, error
payloads, Sentry events, or any other observability sink — not in plain
text and not as a fingerprint.** Move the secrets behind a wrapper type whose
`Debug`/`Display` impls do not expose them, and never include
`Authorization: Bearer …` headers in logged HTTP requests.
