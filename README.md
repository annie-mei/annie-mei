# Annie Mei

A Discord bot written in Rust that fetches anime and manga information from AniList, with theme song lookups powered by MyAnimeList and Spotify.

![Rust](https://img.shields.io/badge/Rust-2024-orange?logo=rust)
![Serenity](https://img.shields.io/badge/Serenity-0.12-blue)
![License](https://img.shields.io/badge/License-GPL--3.0-blue)
![CodeRabbit Pull Request Reviews](https://img.shields.io/coderabbit/prs/github/annie-mei/annie-mei?utm_source=oss&utm_medium=github&utm_campaign=annie-mei%2Fannie-mei&labelColor=171717&color=FF570A&link=https%3A%2F%2Fcoderabbit.ai&label=CodeRabbit+Reviews)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/annie-mei/annie-mei)

## Features

- Fetch detailed anime/manga information from AniList
- Look up opening and ending theme songs with Spotify links
- Link your AniList account with a secure OAuth flow to show guild members' scores
- Check your currently linked AniList account
- Full Japanese kana support for searches

## Commands

| Command | Description |
|---------|-------------|
| `/help` | Shows available commands |
| `/ping` | Bot health check |
| `/register` | Start or refresh the secure AniList OAuth linking flow |
| `/whoami` | Show your linked AniList username and profile link |
| `/anime <search>` | Look up anime by name or AniList ID |
| `/manga <search>` | Look up manga by name or AniList ID |
| `/songs <search>` | Find theme songs for an anime |

### Search Tips

- Use AniList IDs for exact matches
- Japanese kana is supported: `/manga きめつのやいば`
- Wrap numeric titles in quotes: `/songs "86"`

## Infrastructure

| Component | Service |
|-----------|---------|
| Database | Neon (PostgreSQL) |
| Cache | Upstash (Redis) |
| Secrets | Doppler |
