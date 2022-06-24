use super::anilist_manga::Manga;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct FetchResponse {
    pub data: Option<FetchData>,
}

#[derive(Deserialize, Debug)]
pub struct FetchData {
    #[serde(rename = "Media")]
    pub media: Option<Manga>,
}
