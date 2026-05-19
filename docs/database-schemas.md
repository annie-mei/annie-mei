# Database schema ownership

Annie Mei and the auth-service share one Postgres database but own separate schemas so their SQLx migration histories do not conflict.

| Schema | Owner | Tables |
| --- | --- | --- |
| `annie_auth` | auth-service | `oauth_credentials`, `oauth_sessions` |
| `annie_mei` | Annie Mei bot | `user_settings`, `guild_settings` |

Runtime queries should use schema-qualified table names. Do not rely on `search_path` for application reads or writes.

## Migration history

Each service should track new SQLx migrations in its own schema:

- Auth-service startup creates and uses `annie_auth._sqlx_migrations` with `search_path=annie_auth,public`.
- Annie Mei startup migrations create/use `annie_mei._sqlx_migrations` with `search_path=annie_mei,annie_auth,public`.

The bot creates the `annie_mei` schema before SQLx checks migration history so `annie_mei._sqlx_migrations` is schema-local and does not conflict with auth-service migration history. Bot migrations must use SQLx root-level file names like `YYYYMMDDHHMMSS_description.up.sql`; Diesel-style migration directories are ignored by SQLx.

## Destructive reset cutover

ANNIE-189 was deployed as a major-version schema reset. Existing OAuth rows were intentionally discarded, and affected users need to run `/register` again.

Clean reset SQL for the shared app-owned objects:

```sql
DROP TABLE IF EXISTS public.oauth_sessions CASCADE;
DROP TABLE IF EXISTS public.oauth_credentials CASCADE;
DROP TABLE IF EXISTS public._sqlx_migrations CASCADE;

DROP TABLE IF EXISTS annie_auth.oauth_sessions CASCADE;
DROP TABLE IF EXISTS annie_auth.oauth_credentials CASCADE;
DROP TABLE IF EXISTS annie_auth._sqlx_migrations CASCADE;

CREATE SCHEMA IF NOT EXISTS annie_auth;
CREATE SCHEMA IF NOT EXISTS annie_mei;
```

Auth-service startup recreates `annie_auth.oauth_credentials`, `annie_auth.oauth_sessions`, and `annie_auth._sqlx_migrations` from its checked-in migrations.

The bot settings migration now creates bot-owned settings tables directly in `annie_mei`:

```sql
CREATE SCHEMA IF NOT EXISTS annie_mei;

CREATE TABLE IF NOT EXISTS annie_mei.user_settings (...);
CREATE TABLE IF NOT EXISTS annie_mei.guild_settings (...);
```

## Permissions

The auth-service database role should own or fully manage objects in `annie_auth`.

The Annie Mei database role needs cross-schema access for account-link commands:

```sql
GRANT USAGE ON SCHEMA annie_auth TO annie_mei_bot;
GRANT SELECT, DELETE ON annie_auth.oauth_credentials TO annie_mei_bot;
GRANT SELECT, DELETE ON annie_auth.oauth_sessions TO annie_mei_bot;
```

The Annie Mei role also needs read/write access to its own schema:

```sql
GRANT USAGE ON SCHEMA annie_mei TO annie_mei_bot;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA annie_mei TO annie_mei_bot;
```

Use the real runtime role name for each environment. In local/Supabase development this may be `annie` instead of `annie_mei_bot`.

## Deployment order

1. Stop old auth-service and bot instances.
2. Back up the database if any data should be recoverable.
3. Run the destructive reset SQL for legacy OAuth tables and SQLx history.
4. Deploy auth-service so startup creates fresh `annie_auth.*` tables and `annie_auth._sqlx_migrations`.
5. Grant cross-schema permissions for the runtime roles.
6. Deploy Annie Mei code; startup runs bot migrations to create `annie_mei.*` settings tables, reads `annie_auth.*`, and writes `annie_mei.*`.
7. Re-run `/register` for affected users because OAuth credentials were reset.

Avoid public compatibility views for this cutover. They can confuse schema checks and do not safely cover old write paths such as OAuth upserts.

## Rollback

This cutover is destructive for OAuth rows. Roll back application code by redeploying the prior versions and restoring a database backup if the old rows are needed.
