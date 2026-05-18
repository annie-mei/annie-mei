DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'annie_mei' AND c.relname = 'guild_settings' AND c.relkind IN ('r', 'p', 'v', 'm'))
       AND EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relname = 'guild_settings' AND c.relkind IN ('r', 'p', 'v', 'm')) THEN
        RAISE EXCEPTION 'both annie_mei.guild_settings and public.guild_settings exist; resolve manually before reverting';
    ELSIF EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'annie_mei' AND c.relname = 'guild_settings' AND c.relkind IN ('r', 'p')) THEN
        ALTER TABLE annie_mei.guild_settings SET SCHEMA public;
    END IF;

    IF EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'annie_mei' AND c.relname = 'user_settings' AND c.relkind IN ('r', 'p', 'v', 'm'))
       AND EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relname = 'user_settings' AND c.relkind IN ('r', 'p', 'v', 'm')) THEN
        RAISE EXCEPTION 'both annie_mei.user_settings and public.user_settings exist; resolve manually before reverting';
    ELSIF EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'annie_mei' AND c.relname = 'user_settings' AND c.relkind IN ('r', 'p')) THEN
        ALTER TABLE annie_mei.user_settings SET SCHEMA public;
    END IF;
END $$;
