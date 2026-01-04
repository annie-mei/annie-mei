# Annie Mei

A Discord bot written in Rust that fetches anime and manga information from AniList, with theme song lookups powered by MyAnimeList and Spotify.

![Rust](https://img.shields.io/badge/Rust-2024-orange?logo=rust)
![Serenity](https://img.shields.io/badge/Serenity-0.12-blue)
![License](https://img.shields.io/badge/License-GPL--3.0-blue)

## Features

- Fetch detailed anime/manga information from AniList
- Look up opening and ending theme songs with Spotify links
- Link your AniList account to show guild members' scores
- Full Japanese kana support for searches

## Commands

| Command | Description |
|---------|-------------|
| `/help` | Shows available commands |
| `/ping` | Bot health check |
| `/register` | Link your AniList account |
| `/anime <search>` | Look up anime by name or AniList ID |
| `/manga <search>` | Look up manga by name or AniList ID |
| `/songs <search>` | Find theme songs for an anime |

### Search Tips

- Use AniList IDs for exact matches
- Japanese kana is supported: `/manga きめつのやいば`
- Wrap numeric titles in quotes: `/songs "86"`
