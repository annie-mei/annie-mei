use crate::utils::formatter::linker;
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

    fn transform_songs(&self, songs: Option<Vec<SongInfo>>) -> Vec<(String, String, bool)> {
        match songs {
            None => vec![(
                "No information available".to_string(),
                "\u{200b}".to_string(),
                true,
            )],
            Some(songs_list) => {
                let mut songs = vec![];
                for song in songs_list {
                    let song_text = song.text.split("by").collect::<Vec<&str>>();
                    let song_name = song_text[0];
                    let artist_names = song_text[1];
                    songs.push((song_name.to_owned(), artist_names.to_owned(), true));
                }
                songs
            }
        }
    }

    pub fn transform_openings(&self) -> Vec<(String, String, bool)> {
        self.transform_songs(self.opening_themes.clone())
    }

    pub fn transform_endings(&self) -> Vec<(String, String, bool)> {
        self.transform_songs(self.ending_themes.clone())
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
