use super::anime::Anime;
use crate::utils::fuzzy::{fuzzy_matcher, fuzzy_matcher_synonyms};
use log::info;
use serde::Deserialize;

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

    pub fn fuzzy_match(&self, user_input: String) -> Anime {
        let name = user_input.to_lowercase();
        let media_list = &self.filter_anime();
        let english_titles: Vec<String> = media_list
            .iter()
            .map(|media| media.get_english_title())
            .collect();
        let romaji_titles: Vec<String> = media_list
            .iter()
            .map(|media| media.get_romaji_title())
            .collect();
        let synonyms: Vec<Vec<String>> = media_list
            .iter()
            .map(|media| media.get_synonyms())
            .collect();

        let top_english_title_match = fuzzy_matcher(name.clone(), english_titles, 0.5);
        info!(
            "English Title match says: {:#?}",
            media_list[top_english_title_match.as_ref().unwrap().index].get_english_title()
        );
        let top_romaji_title_match = fuzzy_matcher(name.clone(), romaji_titles, 0.5);
        info!(
            "Romaji Title match says: {:#?}",
            media_list[top_romaji_title_match.as_ref().unwrap().index].get_english_title()
        );
        let top_synonym_match = fuzzy_matcher_synonyms(name, synonyms);
        info!(
            "Synonyms match says: {:#?}",
            media_list[top_synonym_match.as_ref().unwrap().index].get_english_title()
        );

        let media_index: usize = match top_english_title_match {
            Some(match_response) => match match_response.result.similarity {
                _ if match_response.result.similarity > 0.90 => match_response.index,
                _ => match top_synonym_match {
                    Some(synonym_match_response) => synonym_match_response.index,
                    None => match_response.index,
                },
            },
            None => todo!(),
        };

        media_list[media_index].clone()
    }
}
