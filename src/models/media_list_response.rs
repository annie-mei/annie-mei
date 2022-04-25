use super::anime::Anime;

#[derive(serde::Deserialize)]
pub struct FetchResponse {
    pub data: Option<Page>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]

pub struct Page {
    pub page_info: Option<PageInfo>,
    #[serde(rename = "media")]
    pub media_list: Option<Vec<Anime>>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub total: Option<u32>,
    pub current_page: Option<u32>,
    pub last_page: Option<u32>,
    pub has_next_page: Option<u32>,
    pub per_page: Option<u32>,
}

impl FetchResponse {
    pub fn fuzzy_match(&self, name: String) -> Anime {
        Anime {
            id: todo!(),
            id_mal: todo!(),
            title: todo!(),
            season: todo!(),
            season_year: todo!(),
            format: todo!(),
            status: todo!(),
            episodes: todo!(),
            duration: todo!(),
            genres: todo!(),
            source: todo!(),
            cover_image: todo!(),
            average_score: todo!(),
            studios: todo!(),
            site_url: todo!(),
            external_links: todo!(),
            trailer: todo!(),
            description: todo!(),
        }
    }
}
