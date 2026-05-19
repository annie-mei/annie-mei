-- Store configurable bot options as one validated setting value per subject.
-- Discord snowflakes are stored as TEXT so they round-trip without lossy casts.
CREATE SCHEMA IF NOT EXISTS annie_mei;

CREATE TABLE IF NOT EXISTS annie_mei.user_settings (
    discord_user_id TEXT NOT NULL,
    setting_key TEXT NOT NULL,
    setting_value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (discord_user_id, setting_key)
);

CREATE TABLE IF NOT EXISTS annie_mei.guild_settings (
    guild_id TEXT NOT NULL,
    setting_key TEXT NOT NULL,
    setting_value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (guild_id, setting_key)
);
