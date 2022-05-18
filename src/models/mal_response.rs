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
    medium: String,
    large: String,
}

#[derive(Deserialize, Debug, Clone)]

struct SongInfo {
    id: u32,
    anime_id: u32,
    text: String,
}
