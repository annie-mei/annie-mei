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
    pub fn no_results(&self) -> bool {
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

        media_list.is_empty()
    }

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

    pub fn fuzzy_match(&self, user_input: &str) -> Option<Anime> {
        let no_result = &self.no_results();

        if *no_result {
            return None;
        }

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

        let top_english_title_match =
            fuzzy_matcher(&*name, english_titles, 0.5).unwrap_or_default();
        let top_romaji_title_match = fuzzy_matcher(&*name, romaji_titles, 0.5).unwrap_or_default();

        let is_english_match_available = top_english_title_match.index != usize::MAX;
        let is_english_match_good = top_english_title_match.result.similarity != 0.85;
        let is_romaji_match_available = top_romaji_title_match.index != usize::MAX;
        let is_romaji_match_good = top_romaji_title_match.result.similarity != 0.85;

        let need_to_match_synonyms = !(is_english_match_available || is_romaji_match_available)
            || !(is_english_match_good || is_romaji_match_good);

        let english_score = top_english_title_match.result.similarity;
        let romaji_score = top_romaji_title_match.result.similarity;
        let top_match = match english_score < romaji_score {
            true => top_romaji_title_match,
            false => top_english_title_match,
        };

        if !need_to_match_synonyms {
            info!(
                "Title match says: {:#?} at Index: {:#?}",
                media_list[top_match.index].get_english_title(),
                top_match.index
            );
            Some(media_list[top_match.index].clone())
        } else {
            let synonyms: Vec<Vec<String>> = media_list
                .iter()
                .map(|media| media.get_synonyms())
                .collect();
            let top_synonym_match = fuzzy_matcher_synonyms(&*name, synonyms).unwrap_or_default();
            match top_synonym_match.index {
                usize::MAX => match top_match.index {
                    usize::MAX => Some(media_list[0].clone()),
                    _ => Some(media_list[top_match.index].clone()),
                },
                _ => {
                    info!(
                        "Synonym match says: {:#?}  at Index: {:#?}",
                        media_list[top_synonym_match.index].get_romaji_title(),
                        top_synonym_match.index
                    );
                    Some(media_list[top_synonym_match.index].clone())
                }
            }
        }
    }
}
