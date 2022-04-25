use super::anime::Anime;

#[derive(serde::Deserialize)]
pub struct FetchResponse {
    pub data: Option<FetchData>,
}

#[derive(serde::Deserialize)]
pub struct FetchData {
    #[serde(rename = "Media")]
    pub media: Option<Anime>,
}
