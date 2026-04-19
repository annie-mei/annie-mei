use crate::{
    models::{
        anilist_common::{CoverImage, ExternalLinks, Tag, Title},
        transformers::Transformers,
    },
    utils::{
        formatter::{code, linker, titlecase},
        statics::{ANILIST_STATUS_RELEASING, EMPTY_STR},
    },
};

use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Anime {
    #[serde(rename = "type")]
    media_type: Option<String>,
    #[allow(dead_code)]
    id: u32,
    id_mal: Option<u32>,
    is_adult: Option<bool>,
    title: Title,
    synonyms: Option<Vec<String>>,
    season: Option<String>,
    season_year: Option<u32>,
    format: Option<String>,
    status: Option<String>,
    episodes: Option<u32>,
    next_airing_episode: Option<NextAiringEpisode>,
    duration: Option<u32>,
    genres: Vec<String>,
    source: Option<String>,
    cover_image: CoverImage,
    average_score: Option<u32>,
    studios: Option<Studios>,
    site_url: String,
    external_links: Option<Vec<ExternalLinks>>,
    trailer: Option<Trailer>,
    description: Option<String>,
    tags: Vec<Tag>,
}

#[derive(Deserialize, Debug, Clone)]

pub struct Studios {
    pub edges: Vec<Edges>,
    pub nodes: Vec<Nodes>,
}
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Edges {
    pub is_main: bool,
}

#[derive(Deserialize, Debug, Clone)]

pub struct Nodes {
    pub name: String,
}

#[derive(Deserialize, Debug, Clone)]

pub struct Trailer {
    pub id: String,
    pub site: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NextAiringEpisode {
    pub episode: Option<u32>,
}

impl Anime {
    pub fn transform_season(&self) -> String {
        let season = match &self.season {
            Some(season) => season.to_string(),
            None => "".to_string(),
        };
        let year = match &self.season_year {
            Some(year) => year.to_string(),
            None => "".to_string(),
        };

        let built_string = [season, year];
        let return_string = titlecase(built_string.join(" ").trim());

        match return_string {
            _ if return_string.is_empty() => EMPTY_STR.to_string(),
            _ => return_string,
        }
    }

    pub fn transform_episodes(&self) -> String {
        if self.status.as_deref() == Some(ANILIST_STATUS_RELEASING)
            && let Some(next_airing_episode) = &self.next_airing_episode
            && let Some(next_episode) = next_airing_episode.episode
        {
            let aired_episodes = next_episode.saturating_sub(1);

            if let Some(total_episodes) = self.episodes {
                return format!("{aired_episodes}/{total_episodes}");
            }

            return aired_episodes.to_string();
        }

        match &self.episodes {
            Some(episodes) => episodes.to_string(),
            None => EMPTY_STR.to_string(),
        }
    }

    pub fn transform_duration(&self) -> String {
        match &self.duration {
            Some(duration) => format!("{duration} mins"),
            None => EMPTY_STR.to_string(),
        }
    }

    pub fn transform_studios(&self) -> String {
        let Some(studios) = self.studios.as_ref() else {
            return EMPTY_STR.to_string();
        };

        if studios.edges.is_empty() || studios.nodes.is_empty() {
            return EMPTY_STR.to_string();
        }

        let mut main_studio_indices: Vec<usize> = studios
            .edges
            .iter()
            .enumerate()
            .filter_map(|(index, edge)| edge.is_main.then_some(index))
            .collect();

        if main_studio_indices.is_empty() {
            main_studio_indices.push(0);
        }

        main_studio_indices
            .into_iter()
            .filter_map(|index| studios.nodes.get(index))
            .map(|node| code(titlecase(&node.name)))
            .collect::<Vec<String>>()
            .join(" x ")
    }
}

impl Transformers for Anime {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_type(&self) -> &str {
        // AniList returns the media type as "ANIME" / "MANGA". Downstream
        // code compares against lower-case constants, so map the known
        // values to static lower-case strings to avoid per-call allocations.
        match self.media_type.as_deref() {
            Some("ANIME") | Some("anime") => "anime",
            Some("MANGA") | Some("manga") => "manga",
            Some(other) => other,
            None => "",
        }
    }

    fn is_adult(&self) -> bool {
        self.is_adult.unwrap_or(false)
    }

    fn get_mal_id(&self) -> Option<u32> {
        self.id_mal
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
        self.source.as_deref()
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
        self.description.as_deref()
    }

    fn get_tags(&self) -> &[Tag] {
        &self.tags
    }

    fn transform_mal_id(&self) -> Option<String> {
        self.id_mal
            .map(|mal_id| format!("https://www.myanimelist.net/anime/{mal_id}"))
    }

    fn transform_season_serialization(&self) -> String {
        self.transform_season()
    }

    fn transform_episodes_chapters(&self) -> String {
        self.transform_episodes()
    }

    fn transform_duration_volumes(&self) -> String {
        self.transform_duration()
    }

    fn transform_studios_staff(&self) -> String {
        self.transform_studios()
    }

    fn transform_links(&self) -> String {
        let Some(links) = &self.external_links else {
            return EMPTY_STR.to_string();
        };

        if links.is_empty() {
            return EMPTY_STR.to_string();
        }

        let parsed_links: String = links
            .iter()
            .filter(|link| link.url_type.to_lowercase() == "streaming")
            .filter_map(|link| {
                let url = link.url.as_str();
                if url.contains("hbo") {
                    Some(linker("HBO", url))
                } else if url.contains("netflix") {
                    Some(linker("Netflix", url))
                } else if url.contains("crunchyroll") {
                    Some(linker("Crunchyroll", url))
                } else {
                    None
                }
            })
            .collect::<Vec<String>>()
            .join(" ");

        if parsed_links.is_empty() {
            EMPTY_STR.to_string()
        } else {
            parsed_links
        }
    }

    fn transform_trailer(&self) -> String {
        match &self.trailer {
            None => String::from("None"),
            Some(trailer) => {
                let url = format!("https://www.{}.com/watch?v={}", trailer.site, trailer.id);
                linker("YouTube", &url)
            }
        }
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

#[cfg(test)]
mod tests {
    use super::Anime;
    use crate::utils::statics::{ANILIST_STATUS_FINISHED, ANILIST_STATUS_RELEASING};
    use serde_json::json;

    fn sample_anime(status: &str, episodes: Option<u32>, next_episode: Option<u32>) -> Anime {
        serde_json::from_value(json!({
            "type": "ANIME",
            "id": 1,
            "idMal": null,
            "isAdult": false,
            "title": {
                "romaji": "Sample",
                "english": "Sample",
                "native": "サンプル"
            },
            "synonyms": null,
            "season": null,
            "seasonYear": null,
            "format": null,
            "status": status,
            "episodes": episodes,
            "nextAiringEpisode": next_episode.map(|episode| json!({ "episode": episode })),
            "duration": null,
            "genres": [],
            "source": null,
            "coverImage": {
                "extraLarge": null,
                "large": null,
                "medium": "https://example.com/image.jpg",
                "color": null
            },
            "averageScore": null,
            "studios": null,
            "siteUrl": "https://anilist.co/anime/1",
            "externalLinks": null,
            "trailer": null,
            "description": null,
            "tags": []
        }))
        .expect("sample anime JSON should deserialize")
    }

    #[test]
    fn transform_episodes_uses_aired_count_for_releasing_anime() {
        let anime = sample_anime(ANILIST_STATUS_RELEASING, Some(12), Some(8));

        assert_eq!(anime.transform_episodes(), "7/12");
    }

    #[test]
    fn transform_episodes_uses_total_for_non_releasing_anime() {
        let anime = sample_anime(ANILIST_STATUS_FINISHED, Some(12), Some(8));

        assert_eq!(anime.transform_episodes(), "12");
    }

    #[test]
    fn transform_episodes_uses_aired_count_when_total_is_unknown() {
        let anime = sample_anime(ANILIST_STATUS_RELEASING, None, Some(8));

        assert_eq!(anime.transform_episodes(), "7");
    }
}
