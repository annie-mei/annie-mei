use crate::utils::formatter::{bold, linker};
use serde::Deserialize;
use std::{collections::HashSet, fmt::Write};

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

impl MalResponse {
    pub fn transform_title(&self) -> String {
        self.title.to_string()
    }

    fn transform_songs(&self, songs: Option<Vec<SongInfo>>) -> String {
        match songs {
            None => "No information available".to_string(),
            Some(mut songs_list) => {
                // Only use first 10 entries, because discord hates large embeds
                songs_list.truncate(10);
                songs_list.shrink_to_fit();
                Self::format_songs_for_display(songs_list)
            }
        }
    }

    pub fn transform_openings(&self) -> String {
        self.transform_songs(self.opening_themes.clone())
    }

    pub fn transform_endings(&self) -> String {
        self.transform_songs(self.ending_themes.clone())
    }

    fn format_songs_for_display(songs: Vec<SongInfo>) -> String {
        let mut return_string: Vec<String> = vec![];
        let mut parsed_songs: HashSet<u32> = HashSet::new();
        for song in songs {
            let song_number = Self::get_song_number(&song.text);
            if parsed_songs.contains(&song_number) {
                continue;
            } else {
                parsed_songs.insert(song_number);
            }
            let song_name = Self::get_song_name(&song.text);
            let artist_names = Self::get_artist_names(&song.text);
            let episode_numbers = Self::get_episode_numbers(&song.text);

            let mut song_string = "".to_string();

            // Add song number
            write!(song_string, "{}. ", song_number).unwrap();

            // Add song name
            write!(song_string, "{}", bold(song_name)).unwrap();

            // Add artist names if they exist
            if artist_names.is_some() {
                write!(song_string, " by {}", artist_names.unwrap()).unwrap();
            }

            // Add episode numbers if they exist
            if episode_numbers.is_some() {
                // Use write
                write!(song_string, " | {}", episode_numbers.unwrap()).unwrap();
            }
            return_string.push(song_string);
        }
        return_string.join("\n")
    }

    fn get_song_number(song: &str) -> u32 {
        let start_index = song.find('#').unwrap();
        let end_index = song.find(':').unwrap();
        song[start_index + 1..end_index].parse::<u32>().unwrap()
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

    fn get_artist_names(song: &str) -> Option<String> {
        let start_index = song.find("by");
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

    pub fn transform_thumbnail(&self) -> String {
        let large = self.main_picture.large.as_ref();
        let medium = self.main_picture.medium.as_ref();

        if let Some(value) = large {
            return value.to_string();
        }

        medium.unwrap().to_string()
    }

    pub fn transform_mal_link(&self) -> String {
        let link = format!("https://www.myanimelist.net/anime/{}", self.id);
        linker("MyAnimeList".to_string(), link)
    }
}
