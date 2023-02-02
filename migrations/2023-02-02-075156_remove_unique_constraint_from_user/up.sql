-- Your SQL goes here
ALTER TABLE users
    DROP CONSTRAINT IF EXISTS users_discord_id_anilist_id_key;
