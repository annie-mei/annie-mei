-- This file should undo anything in `up.sql`
ALTER TABLE users
    ADD CONSTRAINT users_discord_id_anilist_id_key UNIQUE (discord_id, anilist_id);
