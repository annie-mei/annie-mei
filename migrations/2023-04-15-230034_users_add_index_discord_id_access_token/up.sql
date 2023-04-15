-- Your SQL goes here
CREATE INDEX users_discord_id_access_token_index ON users(discord_id, access_token);

