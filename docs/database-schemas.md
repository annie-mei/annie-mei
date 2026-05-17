# Database schema ownership

Annie Mei and the auth-service share one Postgres database but own separate schemas so their SQLx migration histories do not conflict.

| Schema | Owner | Tables |
| --- | --- | --- |
| `auth` | auth-service | `oauth_credentials`, `oauth_sessions` |
| `annie_mei` | Annie Mei bot | `user_settings`, `guild_settings` |

Runtime queries should use schema-qualified table names. Do not rely on `search_path` for application reads or writes.

## Migration history

Each service should track SQLx migrations in its own schema:

- Auth-service startup moves existing public OAuth tables when safe, then sets `search_path` to `auth,public` before running migrations, so SQLx uses `auth._sqlx_migrations`.
- Annie Mei migrations should be run with `search_path` set to `annie_mei,auth,public`, so SQLx uses `annie_mei._sqlx_migrations`.

Create `annie_mei` before running Annie Mei migrations if you want SQLx to create `annie_mei._sqlx_migrations`; SQLx creates/checks its migration table before it runs migration SQL. Do not reconcile or share `public._sqlx_migrations`; it may contain older migrations from either service.

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

1. Deploy auth-service schema changes first, or run its migrations manually, so `auth.oauth_credentials` and `auth.oauth_sessions` exist.
2. Grant Annie Mei access to the `auth` schema and OAuth tables.
3. Create the `annie_mei` schema before running Annie Mei migrations if this deployment should use `annie_mei._sqlx_migrations`.
4. Run Annie Mei migrations with `search_path=annie_mei,auth,public`.
5. Deploy Annie Mei code that reads `auth.*` and writes `annie_mei.*`.

For a short zero-downtime bridge for old bot reads, create compatibility views in `public`. Do not rely on these views for old auth-service writes that use `INSERT ... ON CONFLICT`; deploy auth-service before moving traffic that needs to write OAuth credentials. Drop the views after both services are schema-qualified.

```sql
CREATE OR REPLACE VIEW public.oauth_credentials AS SELECT * FROM auth.oauth_credentials;
CREATE OR REPLACE VIEW public.oauth_sessions AS SELECT * FROM auth.oauth_sessions;
```

Avoid compatibility views for settings writes unless the deployment requires them and they are tested as updatable views.

## Rollback

If schema-qualified code must be rolled back, either keep compatibility views in `public` or move tables back:

```sql
ALTER TABLE IF EXISTS auth.oauth_credentials SET SCHEMA public;
ALTER TABLE IF EXISTS auth.oauth_sessions SET SCHEMA public;
ALTER TABLE IF EXISTS annie_mei.user_settings SET SCHEMA public;
ALTER TABLE IF EXISTS annie_mei.guild_settings SET SCHEMA public;
```

Prefer table moves over copy-and-delete rollback so table metadata and data stay intact.
