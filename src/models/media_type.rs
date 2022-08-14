use strum::AsRefStr;

#[derive(AsRefStr, Debug, Clone)]
pub enum MediaType {
    Anime,
    Manga,
}
