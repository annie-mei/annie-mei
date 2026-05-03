-- Recreate the legacy users table. Existing data cannot be restored from
-- oauth_credentials (no anilist_username stored), so this leaves an empty
-- table that matches the original schema.
CREATE TABLE users (
    discord_id bigint PRIMARY KEY,
    anilist_id bigint NOT NULL,
    anilist_username text NOT NULL
);
CREATE INDEX users_discord_id_anilist_id_index ON users (discord_id, anilist_id);
