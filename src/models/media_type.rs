use super::anilist_anime::Anime;

pub enum MediaType {
    Anime,
    Manga,
}

pub enum MediaResponse {
    Anime(Anime),
    // TODO: Change this to Manga
    Manga(Anime),
}

impl MediaResponse {
    pub fn anime(self) -> Anime {
        if let MediaResponse::Anime(anime) = self {
            anime
        } else {
            panic!("Not a cat")
        }
    }

    pub fn manga(self) -> Anime {
        if let MediaResponse::Manga(manga) = self {
            manga
        } else {
            panic!("Not a cat")
        }
    }
}
