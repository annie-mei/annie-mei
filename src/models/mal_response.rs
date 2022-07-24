use crate::utils::formatter::{bold, linker};
use serde::Deserialize;

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
            Some(songs_list) => Self::format_songs_for_display(songs_list),
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
        for (index, song) in songs.iter().enumerate() {
            let song_name = Self::get_song_name(&song.text);
            let artist_names = Self::get_artist_names(&song.text);
            let episode_numbers = Self::get_episode_numbers(&song.text);
            let song_string = format!(
                "{}. {} by {} | {}",
                index + 1,
                bold(song_name),
                artist_names,
                episode_numbers
            );
            return_string.push(song_string);
        }
        return_string.join("\n")
    }

    fn get_song_name(song: &str) -> String {
        let start_index = song.find('"').unwrap();
        let end_index = song.rfind('"').unwrap();
        song[(start_index + 1)..end_index].to_string()
    }

    fn get_artist_names(song: &str) -> String {
        let start_index = song.find("by").unwrap();
        let end_index = song.rfind('(').unwrap();
        song[(start_index + 3)..end_index].to_string()
    }

    fn get_episode_numbers(song: &str) -> String {
        let start_index = song.rfind('(').unwrap();
        let end_index = song.rfind(')').unwrap();
        song[(start_index + 1)..end_index].to_string()
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
