use super::anime::Anime;
use ngrammatic::{CorpusBuilder, Pad};
use serde::Deserialize;
use tracing::info;

#[derive(Deserialize, Debug)]
pub struct FetchResponse {
    pub data: Option<Page>,
}

#[derive(Deserialize, Debug)]
pub struct Page {
    #[serde(rename = "Page")]
    pub page: Option<PageData>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PageData {
    pub page_info: Option<PageInfo>,
    #[serde(rename = "media")]
    pub media_list: Option<Vec<Anime>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub total: Option<u32>,
    pub current_page: Option<u32>,
    pub last_page: Option<u32>,
    pub has_next_page: Option<bool>,
    pub per_page: Option<u32>,
}

impl FetchResponse {
    pub fn filter_anime(&self) -> Vec<Anime> {
        let media_list = self
            .data
            .as_ref()
            .unwrap()
            .page
            .as_ref()
            .unwrap()
            .media_list
            .as_ref()
            .unwrap()
            .clone();

        media_list
            .iter()
            .filter(|media| media.get_type() == "anime")
            .cloned()
            .collect()
    }

    // Match Using Synonyms
    pub fn fuzzy_match(&self, user_input: String) -> Anime {
        let name = user_input.to_lowercase();
        info!("{:#?}", self);
        let media_list = &self.filter_anime();
        let english_titles: Vec<String> = media_list
            .iter()
            .filter_map(|media| match media.get_type() {
                _ if media.get_type() == "anime" => Some(media.get_english_title()),
                _ => None,
            })
            .collect();

        let mut corpus = CorpusBuilder::new().arity(2).pad_full(Pad::Auto).finish();

        for title in english_titles.iter() {
            corpus.add_text(title)
        }

        let results = corpus.search(&name, 0.25);
        let top_match = results.first();
        info!("English Titles: {:#?}", english_titles);
        info!("Search Results: {:#?}", results);
        info!("Matching Against: {:#?}", name);
        info!("Top Match Sim: {:#?}", top_match.unwrap().similarity);

        let top_match_media_index = english_titles
            .iter()
            .position(|title| *title == top_match.unwrap().text)
            .unwrap();

        media_list[top_match_media_index].clone()
    }
}
