-- Drop the legacy bot-owned users table.
-- After the OAuth /register flow shipped, the auth-service `oauth_credentials`
-- table became the source of truth for AniList account links. The bot no
-- longer writes to `users`; reads have been migrated to `oauth_credentials`.
DROP INDEX IF EXISTS users_discord_id_anilist_id_index;
DROP TABLE IF EXISTS users;
