use crate::{
    models::{
        anilist_common::{CoverImage, Tag, Title},
        transformers::Transformers,
    },
    utils::{formatter::titlecase, statics::EMPTY_STR},
};

use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct RecommendationMediaResponse {
    pub data: Option<RecommendationMediaData>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RecommendationMediaData {
    #[serde(rename = "Media")]
    pub media: Option<RecommendationMedia>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationMedia {
    #[serde(rename = "type")]
    media_type: Option<String>,
    id: u32,
    is_adult: Option<bool>,
    title: Title,
    synonyms: Option<Vec<String>>,
    format: Option<String>,
    status: Option<String>,
    genres: Vec<String>,
    cover_image: CoverImage,
    average_score: Option<u32>,
    site_url: String,
    recommendations: RecommendationConnection,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RecommendationConnection {
    pub nodes: Option<Vec<Option<Recommendation>>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub rating: Option<i32>,
    pub media_recommendation: Option<RecommendedMedia>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedMedia {
    #[serde(rename = "type")]
    media_type: Option<String>,
    is_adult: Option<bool>,
    title: Title,
    format: Option<String>,
    status: Option<String>,
    genres: Vec<String>,
    average_score: Option<u32>,
    site_url: String,
}

impl RecommendationMedia {
    pub fn recommendations(&self) -> impl Iterator<Item = &Recommendation> {
        self.recommendations
            .nodes
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter_map(Option::as_ref)
    }
}

impl Recommendation {
    pub fn recommended_media(&self) -> Option<&RecommendedMedia> {
        self.media_recommendation.as_ref()
    }

    pub fn rating_text(&self) -> String {
        self.rating
            .map_or_else(|| EMPTY_STR.to_string(), |rating| rating.to_string())
    }
}

impl RecommendedMedia {
    pub fn display_title(&self) -> String {
        titlecase(
            self.title
                .english
                .as_deref()
                .or(self.title.romaji.as_deref())
                .or(self.title.native.as_deref())
                .unwrap_or_default(),
        )
    }

    pub fn media_type(&self) -> &str {
        normalize_media_type(self.media_type.as_deref())
    }

    pub fn is_adult(&self) -> bool {
        self.is_adult.unwrap_or(false)
    }

    pub fn format_text(&self) -> &str {
        self.format.as_deref().unwrap_or(EMPTY_STR)
    }

    pub fn status_text(&self) -> &str {
        self.status.as_deref().unwrap_or(EMPTY_STR)
    }

    pub fn average_score(&self) -> Option<u32> {
        self.average_score
    }

    pub fn genres(&self) -> &[String] {
        &self.genres
    }

    pub fn site_url(&self) -> &str {
        &self.site_url
    }
}

impl Transformers for RecommendationMedia {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_type(&self) -> &str {
        normalize_media_type(self.media_type.as_deref())
    }

    fn is_adult(&self) -> bool {
        self.is_adult.unwrap_or(false)
    }

    fn get_mal_id(&self) -> Option<u32> {
        None
    }

    fn get_english_title(&self) -> Option<&str> {
        self.title.english.as_deref()
    }

    fn get_romaji_title(&self) -> Option<&str> {
        self.title.romaji.as_deref()
    }

    fn get_native_title(&self) -> Option<&str> {
        self.title.native.as_deref()
    }

    fn get_synonyms(&self) -> Option<&[String]> {
        self.synonyms.as_deref()
    }

    fn get_format(&self) -> Option<&str> {
        self.format.as_deref()
    }

    fn get_status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    fn get_genres(&self) -> &[String] {
        &self.genres
    }

    fn get_source(&self) -> Option<&str> {
        None
    }

    fn get_cover_image(&self) -> &CoverImage {
        &self.cover_image
    }

    fn get_average_score(&self) -> Option<u32> {
        self.average_score
    }

    fn get_site_url(&self) -> &str {
        &self.site_url
    }

    fn get_description(&self) -> Option<&str> {
        None
    }

    fn get_tags(&self) -> &[Tag] {
        &[]
    }

    fn transform_mal_id(&self) -> Option<String> {
        None
    }

    fn transform_season_serialization(&self) -> String {
        EMPTY_STR.to_string()
    }

    fn transform_episodes_chapters(&self) -> String {
        EMPTY_STR.to_string()
    }

    fn transform_duration_volumes(&self) -> String {
        EMPTY_STR.to_string()
    }

    fn transform_studios_staff(&self) -> String {
        EMPTY_STR.to_string()
    }

    fn transform_links(&self) -> String {
        EMPTY_STR.to_string()
    }

    fn transform_trailer(&self) -> String {
        EMPTY_STR.to_string()
    }

    fn get_season_serialization_text(&self) -> &str {
        "Season"
    }

    fn get_episodes_chapters_text(&self) -> &str {
        "Episodes"
    }

    fn get_duration_volumes_text(&self) -> &str {
        "Duration"
    }

    fn get_studios_staff_text(&self) -> &str {
        "Studios"
    }
}

fn normalize_media_type(media_type: Option<&str>) -> &str {
    match media_type {
        Some("ANIME") | Some("anime") => "anime",
        Some("MANGA") | Some("manga") => "manga",
        Some(other) => other,
        None => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_recommendation_media_response() {
        let response: RecommendationMediaResponse = serde_json::from_value(serde_json::json!({
            "data": {
                "Media": {
                    "type": "ANIME",
                    "id": 1,
                    "isAdult": false,
                    "title": {
                        "romaji": "Cowboy Bebop",
                        "english": "Cowboy Bebop",
                        "native": "カウボーイビバップ"
                    },
                    "synonyms": [],
                    "format": "TV",
                    "status": "FINISHED",
                    "genres": ["Action"],
                    "coverImage": {
                        "extraLarge": "https://example.com/base.jpg",
                        "large": null,
                        "medium": null,
                        "color": "#abcdef"
                    },
                    "averageScore": 86,
                    "siteUrl": "https://anilist.co/anime/1",
                    "recommendations": {
                        "nodes": [{
                            "rating": 42,
                            "mediaRecommendation": {
                                "type": "ANIME",
                                "id": 205,
                                "isAdult": false,
                                "title": {
                                    "romaji": "Samurai Champloo",
                                    "english": "Samurai Champloo",
                                    "native": "サムライチャンプルー"
                                },
                                "format": "TV",
                                "status": "FINISHED",
                                "genres": ["Action", "Adventure"],
                                "coverImage": {
                                    "extraLarge": "https://example.com/recommendation.jpg",
                                    "large": null,
                                    "medium": null,
                                    "color": "#123456"
                                },
                                "averageScore": 84,
                                "siteUrl": "https://anilist.co/anime/205"
                            }
                        }]
                    }
                }
            }
        }))
        .expect("recommendation response should deserialize");

        let media = response.data.unwrap().media.unwrap();
        assert_eq!(media.get_type(), "anime");

        let recommendation = media.recommendations().next().unwrap();
        let recommended_media = recommendation.recommended_media().unwrap();
        assert_eq!(recommendation.rating_text(), "42");
        assert_eq!(recommended_media.display_title(), "Samurai Champloo");
        assert_eq!(recommended_media.media_type(), "anime");
    }
}
