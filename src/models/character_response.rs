use crate::{models::anilist_character::Character, utils::fuzzy::fuzzy_matcher};

use serde::Deserialize;
use tracing::{debug, info};

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
pub struct PageData {
    pub characters: Option<Vec<Character>>,
}

impl FetchResponse {
    pub fn characters(&self) -> Option<&[Character]> {
        self.data.as_ref()?.page.as_ref()?.characters.as_deref()
    }

    pub fn fuzzy_match(&self, user_input: &str) -> Option<Character> {
        let characters = self.characters()?;
        if characters.is_empty() {
            return None;
        }

        let name = user_input.to_lowercase();
        let preferred_names: Vec<String> = characters
            .iter()
            .map(|character| character.name().search_name())
            .collect();
        let top_name_match = fuzzy_matcher(&name, preferred_names, 0.5).unwrap_or_default();

        if top_name_match.index != usize::MAX && top_name_match.result.similarity >= 0.85 {
            info!(
                "Character name match says: {:#?} at Index: {:#?}",
                characters[top_name_match.index].name().search_name(),
                top_name_match.index
            );
            return Some(characters[top_name_match.index].clone());
        }

        let alternative_names: Vec<String> = characters
            .iter()
            .flat_map(|character| character.name().search_aliases())
            .collect();
        let top_alternative_match =
            fuzzy_matcher(&name, alternative_names, 1.0).unwrap_or_default();

        if top_alternative_match.index != usize::MAX {
            let matched_alias = top_alternative_match.result.text;
            debug!("Character alias match says: {matched_alias:#?}");
            return characters
                .iter()
                .find(|character| character.name().has_alias(&matched_alias))
                .cloned();
        }

        match top_name_match.index {
            usize::MAX => characters.first().cloned(),
            index => characters.get(index).cloned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FetchResponse;

    fn character_response_json() -> serde_json::Value {
        serde_json::json!({
            "data": {
                "Page": {
                    "characters": [
                        {
                            "id": 1,
                            "name": {
                                "full": "Monkey D. Luffy",
                                "native": "モンキー・D・ルフィ",
                                "alternative": ["Straw Hat Luffy"],
                                "userPreferred": "Monkey D. Luffy"
                            },
                            "image": { "large": null, "medium": null },
                            "description": null,
                            "gender": null,
                            "dateOfBirth": null,
                            "age": null,
                            "bloodType": null,
                            "favourites": null,
                            "siteUrl": "https://anilist.co/character/1",
                            "media": { "nodes": [] }
                        }
                    ]
                }
            }
        })
    }

    #[test]
    fn empty_response_returns_none() {
        let response: FetchResponse = serde_json::from_value(serde_json::json!({
            "data": { "Page": { "characters": [] } }
        }))
        .expect("payload deserializes");

        assert!(response.fuzzy_match("luffy").is_none());
    }

    #[test]
    fn fuzzy_match_returns_name_match() {
        let response: FetchResponse =
            serde_json::from_value(character_response_json()).expect("payload deserializes");

        let character = response
            .fuzzy_match("Monkey D. Luffy")
            .expect("expected a match");

        assert_eq!(character.transform_name(), "Monkey D. Luffy");
    }

    #[test]
    fn fuzzy_match_returns_alternative_name_match() {
        let response: FetchResponse =
            serde_json::from_value(character_response_json()).expect("payload deserializes");

        let character = response
            .fuzzy_match("Straw Hat Luffy")
            .expect("expected a match");

        assert_eq!(character.transform_name(), "Monkey D. Luffy");
    }
}
