# Annie Mei

A Discord bot written in Rust that fetches anime and manga information from AniList, with theme song lookups powered by MyAnimeList and Spotify.

![Rust](https://img.shields.io/badge/Rust-2024-orange?logo=rust)
![Serenity](https://img.shields.io/badge/Serenity-0.12-blue)
![License](https://img.shields.io/badge/License-GPL--3.0-blue)
![CodeRabbit Pull Request Reviews](https://img.shields.io/coderabbit/prs/github/annie-mei/annie-mei?utm_source=oss&utm_medium=github&utm_campaign=annie-mei%2Fannie-mei&labelColor=171717&color=FF570A&link=https%3A%2F%2Fcoderabbit.ai&label=CodeRabbit+Reviews)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/annie-mei/annie-mei)

## Features

- Fetch detailed anime/manga/character information from AniList
- Use Gemini to turn natural-language searches into anime/manga lookups
- Look up opening and ending theme songs with Spotify links
- Link your AniList account with a secure OAuth flow to show guild members' scores
- Check or unlink your currently linked AniList account
- Full Japanese kana support for searches

## Commands

| Command | Description |
|---------|-------------|
| `/help` | Shows available commands |
| `/ping` | Bot health check |
| `/register` | Start or refresh the secure AniList OAuth linking flow |
| `/unregister confirmation:<confirm\|cancel>` | Unlink your AniList account after confirmation |
| `/whoami` | Show your linked AniList account ID and profile link |
| `/anime <search>` | Look up anime by name or AniList ID |
| `/manga <search>` | Look up manga by name or AniList ID |
| `/search <query>` | Search for anime or manga using natural language powered by Gemini |
| `/character search:<term or id> spoilers:<allow\|disallow>` | Look up characters by name or AniList ID |
| `/songs <search>` | Find theme songs for an anime |
| `/settings` | Open an interactive panel showing your current user, guild, and default settings |

### Search Tips

- Use AniList IDs for exact matches
- Use natural language when you do not know the exact title: `/search anime about volleyball`
- Japanese kana is supported: `/manga きめつのやいば`
- Wrap numeric titles in quotes: `/songs "86"`

### Settings

Run `/settings` with no arguments to open Annie Mei's interactive settings panel. The panel summarizes your effective preferences, your user override, the server override when available, and the default for each setting.

Current settings shown in the panel:

- Title display: preferred AniList title variant (`matched`, `romaji`, `english`, or `native`)
- Analytics privacy: whether raw user-provided content can be included in supported analytics (`standard` or `opted_out`)
- Guild scores: whether server score displays are enabled and whether you participate (`enabled`, `disabled`, or `opted_out`)

## Infrastructure

| Component | Service |
|-----------|---------|
| Database | Supabase (PostgreSQL) |
| Cache | Upstash (Redis) |
| Secrets | Doppler |

## Documentation

- [OAuth data contract](docs/oauth-contract.md) — shared schema and payload contract with the [`auth`](https://github.com/annie-mei/auth) repo
- [Database schema ownership](docs/database-schemas.md) — shared Postgres schemas, migration isolation, grants, and deployment order

## Legal

- [Terms of Service](TERMS_OF_SERVICE.md)
- [Privacy Policy](PRIVACY_POLICY.md)
