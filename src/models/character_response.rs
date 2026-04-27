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

    pub fn fuzzy_match(&self, user_input: &str, allow_spoilers: bool) -> Option<Character> {
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
            .flat_map(|character| character.name().search_aliases(allow_spoilers))
            .collect();
        let top_alternative_match =
            fuzzy_matcher(&name, alternative_names, 1.0).unwrap_or_default();

        if top_alternative_match.index != usize::MAX {
            let matched_alias = top_alternative_match.result.text;
            debug!("Character alias match says: {matched_alias:#?}");
            return characters
                .iter()
                .find(|character| character.name().has_alias(&matched_alias, allow_spoilers))
                .cloned();
        }

        if !allow_spoilers
            && characters
                .iter()
                .any(|character| character.name().has_spoiler_alias(user_input))
        {
            info!("Character spoiler alias matched while spoilers are disallowed");
            return None;
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
                                "alternativeSpoiler": ["Joy Boy"],
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

        assert!(response.fuzzy_match("luffy", false).is_none());
    }

    #[test]
    fn fuzzy_match_returns_name_match() {
        let response: FetchResponse =
            serde_json::from_value(character_response_json()).expect("payload deserializes");

        let character = response
            .fuzzy_match("Monkey D. Luffy", false)
            .expect("expected a match");

        assert_eq!(character.transform_name(), "Monkey D. Luffy");
    }

    #[test]
    fn fuzzy_match_returns_alternative_name_match() {
        let response: FetchResponse =
            serde_json::from_value(character_response_json()).expect("payload deserializes");

        let character = response
            .fuzzy_match("Straw Hat Luffy", false)
            .expect("expected a match");

        assert_eq!(character.transform_name(), "Monkey D. Luffy");
    }

    #[test]
    fn fuzzy_match_requires_spoiler_allowance_for_spoiler_aliases() {
        let response: FetchResponse =
            serde_json::from_value(character_response_json()).expect("payload deserializes");

        assert!(response.fuzzy_match("Joy Boy", false).is_none());

        let character = response
            .fuzzy_match("Joy Boy", true)
            .expect("expected a spoiler alias match");

        assert_eq!(character.transform_name(), "Monkey D. Luffy");
    }
}
