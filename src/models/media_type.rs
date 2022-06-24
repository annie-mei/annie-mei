use super::{anilist_anime::Anime, anilist_manga::Manga};

pub enum MediaType {
    Anime,
    Manga,
}

pub enum MediaResponse {
    Anime(Anime),
    // TODO: Change this to Manga
    Manga(Manga),
}

impl MediaResponse {
    pub fn anime(self) -> Anime {
        if let MediaResponse::Anime(anime) = self {
            anime
        } else {
            panic!("Not an Anime")
        }
    }

    pub fn manga(self) -> Manga {
        if let MediaResponse::Manga(manga) = self {
            manga
        } else {
            panic!("Not a Manga")
        }
    }
}
