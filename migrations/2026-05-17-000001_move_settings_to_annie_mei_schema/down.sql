DO $$
BEGIN
    IF to_regclass('annie_mei.guild_settings') IS NOT NULL
       AND to_regclass('public.guild_settings') IS NOT NULL THEN
        RAISE EXCEPTION 'both annie_mei.guild_settings and public.guild_settings exist; resolve manually before reverting';
    ELSIF to_regclass('annie_mei.guild_settings') IS NOT NULL THEN
        ALTER TABLE annie_mei.guild_settings SET SCHEMA public;
    END IF;

    IF to_regclass('annie_mei.user_settings') IS NOT NULL
       AND to_regclass('public.user_settings') IS NOT NULL THEN
        RAISE EXCEPTION 'both annie_mei.user_settings and public.user_settings exist; resolve manually before reverting';
    ELSIF to_regclass('annie_mei.user_settings') IS NOT NULL THEN
        ALTER TABLE annie_mei.user_settings SET SCHEMA public;
    END IF;
END $$;
