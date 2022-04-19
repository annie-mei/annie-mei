use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Anime {
    pub id: u32,
    pub title: Title,
}

#[derive(Deserialize, Debug)]
pub struct Title {
    pub romaji: String,
    pub english: String,
    pub native: String,
}
