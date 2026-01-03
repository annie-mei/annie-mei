use crate::{
    models::{
        anilist_common::{CoverImage, ExternalLinks, Tag, Title},
        transformers::Transformers,
    },
    utils::{
        formatter::{code, linker, titlecase},
        statics::EMPTY_STR,
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
    title: Title,
    synonyms: Option<Vec<String>>,
    season: Option<String>,
    season_year: Option<u32>,
    format: Option<String>,
    status: Option<String>,
    episodes: Option<u32>,
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
        if self.studios.is_none() {
            return EMPTY_STR.to_string();
        }

        let studios = &self.studios.as_ref().unwrap();

        if studios.edges.is_empty() || studios.nodes.is_empty() {
            return EMPTY_STR.to_string();
        }

        let mut main_studio_indices: Vec<usize> = Vec::new();

        for (index, edge) in studios.edges.iter().enumerate() {
            if edge.is_main {
                main_studio_indices.push(index);
            }
        }

        if main_studio_indices.is_empty() {
            main_studio_indices.push(0_usize);
        }

        let mut main_studios: Vec<String> = Vec::new();

        for main_studio_index in main_studio_indices {
            main_studios.push(studios.nodes[main_studio_index].name.to_string())
        }

        let main_studios = main_studios
            .clone()
            .into_iter()
            .map(|studio| code(titlecase(&studio)))
            .collect::<Vec<String>>();

        main_studios.join(" x ")
    }
}

impl Transformers for Anime {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_type(&self) -> String {
        self.media_type.as_ref().unwrap().to_string().to_lowercase()
    }

    fn get_mal_id(&self) -> Option<u32> {
        self.id_mal
    }

    fn get_english_title(&self) -> Option<String> {
        self.title.english.to_owned()
    }

    fn get_romaji_title(&self) -> Option<String> {
        self.title.romaji.to_owned()
    }

    fn get_native_title(&self) -> Option<String> {
        self.title.native.to_owned()
    }

    fn get_synonyms(&self) -> Option<Vec<String>> {
        self.synonyms.to_owned()
    }

    fn get_format(&self) -> Option<String> {
        self.format.to_owned()
    }

    fn get_status(&self) -> Option<String> {
        self.status.to_owned()
    }

    fn get_genres(&self) -> Vec<String> {
        self.genres.to_owned()
    }

    fn get_source(&self) -> Option<String> {
        self.source.to_owned()
    }

    fn get_cover_image(&self) -> CoverImage {
        self.cover_image.to_owned()
    }

    fn get_average_score(&self) -> Option<u32> {
        self.average_score.to_owned()
    }

    fn get_site_url(&self) -> String {
        self.site_url.to_owned()
    }

    fn get_description(&self) -> Option<String> {
        self.description.to_owned()
    }

    fn get_tags(&self) -> Vec<Tag> {
        self.tags.to_owned()
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
        let return_string: String = match &self.external_links {
            Some(links) => {
                if links.is_empty() {
                    EMPTY_STR.to_string()
                } else {
                    let parsed_links = links
                        .iter()
                        .filter(|link| link.url_type.to_lowercase() == "streaming")
                        .map(|link| link.url.to_string())
                        .collect::<Vec<String>>()
                        .into_iter()
                        .filter(|link| match link {
                            _ if link.contains("hbo") => true,
                            _ if link.contains("netflix") => true,
                            _ if link.contains("crunchyroll") => true,
                            _ => false,
                        })
                        .collect::<Vec<String>>()
                        .into_iter()
                        .map(|link| match link {
                            _ if link.contains("hbo") => linker("HBO".to_string(), link),
                            _ if link.contains("netflix") => linker("Netflix".to_string(), link),
                            _ if link.contains("crunchyroll") => {
                                linker("Crunchyroll".to_string(), link)
                            }
                            _ => "Invalid".to_string(),
                        })
                        .collect::<Vec<String>>()
                        .join(" ");
                    if !parsed_links.is_empty() {
                        parsed_links
                    } else {
                        EMPTY_STR.to_string()
                    }
                }
            }
            None => EMPTY_STR.to_string(),
        };

        return_string
    }

    fn transform_trailer(&self) -> String {
        match &self.trailer {
            None => String::from("None"),
            Some(trailer) => {
                let url: String =
                    format!("https://www.{}.com/watch?v={}", trailer.site, trailer.id);
                linker("YouTube".to_string(), url)
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
