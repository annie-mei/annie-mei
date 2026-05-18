# Database schema ownership

Annie Mei and the auth-service share one Postgres database but own separate schemas so their SQLx migration histories do not conflict.

| Schema | Owner | Tables |
| --- | --- | --- |
| `auth` | auth-service | `oauth_credentials`, `oauth_sessions` |
| `annie_mei` | Annie Mei bot | `user_settings`, `guild_settings` |

Runtime queries should use schema-qualified table names. Do not rely on `search_path` for application reads or writes.

## Migration history

Each service should track new SQLx migrations in its own schema:

- Auth-service startup moves existing public OAuth tables when safe, then sets `search_path` to `auth,public` before running migrations, so SQLx uses `auth._sqlx_migrations`.
- Fresh Annie Mei installs can run migrations with `search_path` set to `annie_mei,auth,public`, so SQLx uses `annie_mei._sqlx_migrations`.

Create `annie_mei` before running Annie Mei migrations if you want SQLx to create `annie_mei._sqlx_migrations`; SQLx creates/checks its migration table before it runs migration SQL.

For existing deployments with prior Annie Mei rows in `public._sqlx_migrations`, do not switch directly to `search_path=annie_mei,auth,public`. That makes SQLx replay historical migrations into `annie_mei` before the table-move migration runs, which can create duplicate empty tables and stop the migration. Instead, first apply the schema-move migration with the existing migration-history location, then seed `annie_mei._sqlx_migrations` with only Annie Mei migration rows before using schema-local migration history.

Example SQLx-history cutover after the Annie Mei schema-move migration has run:

```sql
CREATE SCHEMA IF NOT EXISTS annie_mei;
CREATE TABLE IF NOT EXISTS annie_mei._sqlx_migrations (LIKE public._sqlx_migrations INCLUDING ALL);

INSERT INTO annie_mei._sqlx_migrations
SELECT *
FROM public._sqlx_migrations
WHERE description IN (
    'diesel_initial_setup',
    'create_users',
    'remove_unique_constraint_from_user',
    'users_add_index_discord_id_anilist_id',
    'drop_users_table',
    'create_settings_tables',
    'move_settings_to_annie_mei_schema'
)
ON CONFLICT (version) DO NOTHING;
```

## Existing data move

Move existing public tables into their owner schema instead of copying data. `ALTER TABLE ... SET SCHEMA` preserves rows, indexes, constraints, and defaults.

```sql
CREATE SCHEMA IF NOT EXISTS auth;
CREATE SCHEMA IF NOT EXISTS annie_mei;

ALTER TABLE IF EXISTS public.oauth_credentials SET SCHEMA auth;
ALTER TABLE IF EXISTS public.oauth_sessions SET SCHEMA auth;
ALTER TABLE IF EXISTS public.user_settings SET SCHEMA annie_mei;
ALTER TABLE IF EXISTS public.guild_settings SET SCHEMA annie_mei;
```

Auth-service startup performs the OAuth table move before SQLx runs migrations, and the auth-service also has a forward migration for direct SQLx tooling and local tests. Annie Mei has a forward migration that moves `public.user_settings` and `public.guild_settings` into `annie_mei`, and it fails if both schemas contain the same table.

## Permissions

The Annie Mei database role needs:

```sql
GRANT USAGE ON SCHEMA auth TO annie_mei_bot;
GRANT SELECT, DELETE ON auth.oauth_credentials TO annie_mei_bot;
GRANT SELECT, DELETE ON auth.oauth_sessions TO annie_mei_bot;

GRANT USAGE ON SCHEMA annie_mei TO annie_mei_bot;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA annie_mei TO annie_mei_bot;
```

The auth-service database role should own or fully manage objects in `auth`.

## Deployment order

1. Drain or stop old auth-service instances before moving OAuth tables. Old auth-service code uses unqualified public table names and is not safe to overlap with the table move.
2. Deploy auth-service schema changes first, or run its migrations manually, so `auth.oauth_credentials` and `auth.oauth_sessions` exist.
3. Grant Annie Mei access to the `auth` schema and OAuth tables.
4. For existing databases, run Annie Mei migrations once with the current migration-history location so the table-move migration can move `public.user_settings` and `public.guild_settings` before any historical migrations are replayed in `annie_mei`.
5. If Annie Mei should use `annie_mei._sqlx_migrations` after that cutover, create `annie_mei._sqlx_migrations` from the existing Annie Mei migration rows only. Do not copy auth-service migration rows into Annie Mei's migration table.
6. Deploy Annie Mei code that reads `auth.*` and writes `annie_mei.*`.

Avoid public compatibility views for this cutover. They can confuse startup duplicate checks and do not safely cover old write paths such as OAuth upserts.

## Rollback

If schema-qualified code must be rolled back, move tables back:

```sql
ALTER TABLE IF EXISTS auth.oauth_credentials SET SCHEMA public;
ALTER TABLE IF EXISTS auth.oauth_sessions SET SCHEMA public;
ALTER TABLE IF EXISTS annie_mei.user_settings SET SCHEMA public;
ALTER TABLE IF EXISTS annie_mei.guild_settings SET SCHEMA public;
```

Prefer table moves over copy-and-delete rollback so table metadata and data stay intact.
