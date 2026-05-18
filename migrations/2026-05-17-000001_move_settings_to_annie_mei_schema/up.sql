CREATE SCHEMA IF NOT EXISTS annie_mei;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relname = 'user_settings' AND c.relkind IN ('r', 'p', 'v', 'm'))
       AND EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'annie_mei' AND c.relname = 'user_settings' AND c.relkind IN ('r', 'p', 'v', 'm')) THEN
        RAISE EXCEPTION 'both public.user_settings and annie_mei.user_settings exist; resolve manually before continuing';
    ELSIF EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relname = 'user_settings' AND c.relkind IN ('r', 'p')) THEN
        ALTER TABLE public.user_settings SET SCHEMA annie_mei;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relname = 'guild_settings' AND c.relkind IN ('r', 'p', 'v', 'm'))
       AND EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'annie_mei' AND c.relname = 'guild_settings' AND c.relkind IN ('r', 'p', 'v', 'm')) THEN
        RAISE EXCEPTION 'both public.guild_settings and annie_mei.guild_settings exist; resolve manually before continuing';
    ELSIF EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relname = 'guild_settings' AND c.relkind IN ('r', 'p')) THEN
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
