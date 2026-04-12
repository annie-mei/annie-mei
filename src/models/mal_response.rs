use crate::utils::formatter::{bold, linker};

use std::{collections::HashSet, fmt::Write};

use serde::Deserialize;
use tracing::instrument;

#[derive(Deserialize, Debug, Clone)]
pub struct MalResponse {
    id: u32,
    title: String,
    main_picture: MalPicture,
    opening_themes: Option<Vec<SongInfo>>,
    ending_themes: Option<Vec<SongInfo>>,
}

#[derive(Deserialize, Debug, Clone)]
struct MalPicture {
    medium: Option<String>,
    large: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct SongInfo {
    #[allow(dead_code)]
    id: u32,
    #[allow(dead_code)]
    anime_id: u32,
    text: String,
}

/// A song parsed from a MAL response with metadata extracted from the raw text.
/// The `spotify_url` field is left as `None` during parsing and filled in
/// separately by the Spotify enrichment step.
#[derive(Debug, Clone)]
pub struct ParsedSong {
    pub display_number: u32,
    pub song_name: String,
    pub romaji_name: String,
    pub kana_name: Option<String>,
    pub artist_names: Option<String>,
    pub episode_numbers: Option<String>,
    pub spotify_url: Option<String>,
}

impl MalResponse {
    /// Parse opening themes into [`ParsedSong`] values without performing any I/O.
    #[instrument(name = "mal_response.parse_openings", skip(self))]
    pub fn parse_openings(&self) -> Vec<ParsedSong> {
        Self::truncate_and_parse(self.opening_themes.clone())
    }

    /// Parse ending themes into [`ParsedSong`] values without performing any I/O.
    #[instrument(name = "mal_response.parse_endings", skip(self))]
    pub fn parse_endings(&self) -> Vec<ParsedSong> {
        Self::truncate_and_parse(self.ending_themes.clone())
    }

    fn truncate_and_parse(songs: Option<Vec<SongInfo>>) -> Vec<ParsedSong> {
        match songs {
            None => vec![],
            Some(mut songs_list) => {
                // Only use first 10 entries, because discord hates large embeds
                songs_list.truncate(10);
                Self::parse_songs(songs_list)
            }
        }
    }

    fn parse_songs(songs: Vec<SongInfo>) -> Vec<ParsedSong> {
        let mut result = vec![];
        let mut seen_numbers: HashSet<u32> = HashSet::new();

        for (index, song) in songs.iter().enumerate() {
            let song_number = Self::get_song_number(&song.text);

            if let Some(num) = song_number {
                if seen_numbers.contains(&num) {
                    continue;
                }
                seen_numbers.insert(num);
            }

            let song_name = Self::get_song_name(&song.text);
            let romaji_name = Self::get_romaji_song_name(&song_name);
            let kana_name = Self::get_kana_song_name(&song_name);
            let artist_names = Self::get_artist_names(&song.text);
            let episode_numbers = Self::get_episode_numbers(&song.text);

            result.push(ParsedSong {
                display_number: song_number.unwrap_or((index + 1) as u32),
                song_name,
                romaji_name,
                kana_name,
                artist_names,
                episode_numbers,
                spotify_url: None,
            });
        }

        result
    }

    /// Format a slice of [`ParsedSong`] values into the Discord display string.
    /// Spotify URLs, if present, are rendered as hyperlinks on the song name.
    #[instrument(name = "mal_response.format_parsed_songs", skip(songs))]
    pub fn format_parsed_songs(songs: &[ParsedSong]) -> String {
        if songs.is_empty() {
            return "No information available".to_string();
        }

        let mut lines: Vec<String> = Vec::with_capacity(songs.len());

        for song in songs {
            let mut line = String::new();
            write!(line, "{}. ", song.display_number).unwrap();

            match &song.spotify_url {
                Some(url) => {
                    write!(
                        line,
                        "{}",
                        linker(bold(song.song_name.clone()), url.clone())
                    )
                    .unwrap();
                }
                None => {
                    write!(line, "{}", song.song_name).unwrap();
                }
            }

            if let Some(ref artists) = song.artist_names {
                write!(line, " by {}", artists).unwrap();
            }

            if let Some(ref episodes) = song.episode_numbers {
                write!(line, " | {}", episodes).unwrap();
            }

            lines.push(line);
        }

        lines.join("\n")
    }

    fn get_artist_names(song: &str) -> Option<String> {
        let start_index = song.find("by");

        // skipcq: RS-W1031
        let end_index = song.rfind('(').unwrap_or(song.len());
        // If there is no "by" in the song, then there are no artists
        start_index?;
        let start_index = start_index.unwrap();

        // The case when the response overflows into multiple api response
        let artist_names = if end_index < start_index {
            song[(start_index + 3)..].to_string()
        } else {
            song[(start_index + 3)..end_index].to_string()
        };
        // +3 to skip the "by" and the space after it
        let number_of_artists = artist_names.split('&').count();

        if number_of_artists > 3 {
            let mut artist_names = artist_names.split('&').take(3).collect::<Vec<&str>>();
            artist_names.push("and more");
            Some(artist_names.join(", "))
        } else {
            Some(artist_names)
        }
    }

    fn get_episode_numbers(song: &str) -> Option<String> {
        let has_episodes_numbers = song.contains("(ep");
        if !has_episodes_numbers {
            return None;
        }
        let start_index = song.rfind('(').unwrap();
        let end_index = song.rfind(')').unwrap();
        Some(song[(start_index + 1)..end_index].to_string())
    }

    fn get_song_name(song: &str) -> String {
        let start_index = song
            .find('"')
            .unwrap_or_else(|| song.find('\'').unwrap_or(usize::MAX));
        let end_index = song
            .rfind('"')
            .unwrap_or_else(|| song.rfind('\'').unwrap_or(usize::MAX));

        if start_index == usize::MAX || end_index == usize::MAX {
            return "No information available".to_string();
        }
        song[(start_index + 1)..end_index].to_string()
    }

    fn get_romaji_song_name(song_name: &str) -> String {
        let end_index = song_name.find('(').unwrap_or(usize::MAX);

        if end_index == usize::MAX {
            return song_name.to_string();
        }
        song_name[..end_index].to_string()
    }

    fn get_kana_song_name(song_name: &str) -> Option<String> {
        let start_index = song_name.find('(').unwrap_or(usize::MAX);
        let end_index = song_name.find(')').unwrap_or(usize::MAX);

        if start_index == usize::MAX || end_index == usize::MAX {
            return None;
        }
        Some(song_name[(start_index + 1)..end_index].to_string())
    }

    fn get_song_number(song: &str) -> Option<u32> {
        let has_song_number = song.contains('#');
        if !has_song_number {
            return None;
        }

        let start_index = song.find('#')?;
        let end_index = song.find(':')?;

        song[start_index + 1..end_index].parse::<u32>().ok()
    }

    pub fn transform_mal_link(&self) -> String {
        let link = format!("https://www.myanimelist.net/anime/{}", self.id);
        linker("MyAnimeList".to_string(), link)
    }

    pub fn transform_thumbnail(&self) -> String {
        let large = self.main_picture.large.as_ref();
        let medium = self.main_picture.medium.as_ref();

        if let Some(value) = large {
            return value.to_string();
        }

        medium.unwrap().to_string()
    }

    pub fn transform_title(&self) -> String {
        self.title.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::MalResponse;

    #[test]
    fn get_song_number_parses_numeric_prefixes() {
        let song = "#1: \"Again\" by YUI";

        assert_eq!(MalResponse::get_song_number(song), Some(1));
    }

    #[test]
    fn get_song_number_ignores_non_numeric_prefixes() {
        let song = "#TV: \"Again\" by YUI";

        assert_eq!(MalResponse::get_song_number(song), None);
    }
}
