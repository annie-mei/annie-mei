-- Your SQL goes here
CREATE TABLE users (
    discord_id bigint PRIMARY KEY,
    anilist_id bigint NOT NULL,
    anilist_username text NOT NULL,
    UNIQUE (discord_id, anilist_id))
