-- Your SQL goes here
CREATE TABLE users (
  discord_id BIGINT PRIMARY KEY,
  anilist_id BIGINT NOT NULL,
  anilist_username TEXT NOT NULL,
  UNIQUE(discord_id, anilist_id)
)
