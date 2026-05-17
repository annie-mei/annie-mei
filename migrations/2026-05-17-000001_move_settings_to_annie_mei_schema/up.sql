CREATE SCHEMA IF NOT EXISTS annie_mei;

DO $$
BEGIN
    IF to_regclass('public.user_settings') IS NOT NULL
       AND to_regclass('annie_mei.user_settings') IS NOT NULL THEN
        RAISE EXCEPTION 'both public.user_settings and annie_mei.user_settings exist; resolve manually before continuing';
    ELSIF to_regclass('public.user_settings') IS NOT NULL THEN
        ALTER TABLE public.user_settings SET SCHEMA annie_mei;
    END IF;

    IF to_regclass('public.guild_settings') IS NOT NULL
       AND to_regclass('annie_mei.guild_settings') IS NOT NULL THEN
        RAISE EXCEPTION 'both public.guild_settings and annie_mei.guild_settings exist; resolve manually before continuing';
    ELSIF to_regclass('public.guild_settings') IS NOT NULL THEN
        ALTER TABLE public.guild_settings SET SCHEMA annie_mei;
    END IF;
END $$;

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
