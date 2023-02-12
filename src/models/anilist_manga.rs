use crate::{
    models::{
        anilist_common::{CoverImage, ExternalLinks, Tag, Title},
        transformers::Transformers,
    },
    utils::{formatter::code, statics::EMPTY_STR},
};

use chrono::NaiveDate;
use serde::Deserialize;
use titlecase::titlecase;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Manga {
    #[serde(rename = "type")]
    media_type: Option<String>,
    #[allow(dead_code)]
    id: u32,
    id_mal: Option<u32>,
    title: Title,
    synonyms: Option<Vec<String>>,
    start_date: Option<AnilistDate>,
    end_date: Option<AnilistDate>,
    format: Option<String>,
    status: Option<String>,
    chapters: Option<u32>,
    volumes: Option<u32>,
    genres: Vec<String>,
    source: Option<String>,
    cover_image: CoverImage,
    average_score: Option<u32>,
    staff: Option<Staff>,
    site_url: String,
    #[allow(dead_code)]
    external_links: Option<Vec<ExternalLinks>>,
    description: Option<String>,
    tags: Vec<Tag>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AnilistDate {
    pub year: Option<u32>,
    pub month: Option<u32>,
    pub day: Option<u32>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Staff {
    pub edges: Vec<Edges>,
    pub nodes: Vec<Nodes>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Edges {
    pub id: u32,
    pub role: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Nodes {
    pub id: u32,
    pub name: StaffName,
    pub site_url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StaffName {
    pub full: String,
}

impl Manga {
    pub fn transform_date(&self) -> String {
        let start_date = self.start_date.as_ref().unwrap();
        let formatted_start_date = get_formatted_date_string(start_date);

        let is_end_date_available = if let Some(end_date) = &self.end_date {
            end_date.year.is_some() && end_date.month.is_some()
        } else {
            false
        };

        if is_end_date_available {
            let end_date = self.end_date.as_ref().unwrap();
            let formatted_end_date = get_formatted_date_string(end_date);
            format!("{formatted_start_date} - {formatted_end_date}")
        } else {
            formatted_start_date
        }
    }

    pub fn transform_chapters(&self) -> String {
        match &self.chapters {
            Some(chapters) => chapters.to_string(),
            None => EMPTY_STR.to_string(),
        }
    }

    pub fn transform_volumes(&self) -> String {
        match &self.volumes {
            Some(volumes) => volumes.to_string(),
            None => EMPTY_STR.to_string(),
        }
    }

    pub fn transform_staff(&self) -> String {
        if self.staff.is_none() {
            return EMPTY_STR.to_string();
        }

        let staff = &self.staff.as_ref().unwrap();

        if staff.edges.is_empty() || staff.nodes.is_empty() {
            return EMPTY_STR.to_string();
        }

        let mut mangaka_index = 0_usize;
        let mut artist_index = 0_usize;

        for (index, edge) in staff.edges.iter().enumerate() {
            if edge.role.to_lowercase().contains("story") {
                mangaka_index = index;
            }
            if edge.role.to_lowercase().contains("art") {
                artist_index = index;
            }
        }

        let mangaka_name = staff.nodes[mangaka_index].name.full.to_string();
        let artist_name = staff.nodes[artist_index].name.full.to_string();

        if mangaka_name == artist_name {
            code(titlecase(&mangaka_name))
        } else {
            format!(
                "{} x {}",
                code(titlecase(&mangaka_name)),
                code(titlecase(&artist_name))
            )
        }
    }
}

fn get_formatted_date_string(date: &AnilistDate) -> String {
    match date.day {
        Some(day) => {
            let date_string = NaiveDate::from_ymd_opt(
                date.year.unwrap().try_into().unwrap(),
                date.month.unwrap(),
                day,
            );
            if let Some(date_string) = date_string {
                date_string.format("%b %e %Y").to_string()
            } else {
                EMPTY_STR.to_string()
            }
        }
        None => {
            if let Some(month) = date.month {
                let date_string = NaiveDate::from_ymd_opt(
                    date.year.unwrap().try_into().unwrap(),
                    month,
                    // Need to use 1 as the day to give NaiveDate a valid date
                    1,
                );
                if let Some(date_string) = date_string {
                    date_string.format("%b %Y").to_string()
                } else {
                    EMPTY_STR.to_string()
                }
            } else {
                let date_string = NaiveDate::from_ymd_opt(
                    date.year.unwrap().try_into().unwrap(),
                    // Need to use 1 as the month to give NaiveDate a valid date
                    1,
                    // Need to use 1 as the day to give NaiveDate a valid date
                    1,
                );
                if let Some(date_string) = date_string {
                    date_string.format("%Y").to_string()
                } else {
                    EMPTY_STR.to_string()
                }
            }
        }
    }
}

impl Transformers for Manga {
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
            .map(|mal_id| format!("https://www.myanimelist.net/manga/{mal_id}"))
    }

    fn transform_season_serialization(&self) -> String {
        self.transform_date()
    }

    fn transform_episodes_chapters(&self) -> String {
        self.transform_chapters()
    }

    fn transform_duration_volumes(&self) -> String {
        self.transform_volumes()
    }

    fn transform_studios_staff(&self) -> String {
        self.transform_staff()
    }

    fn transform_links(&self) -> String {
        EMPTY_STR.to_string()
    }

    fn transform_trailer(&self) -> String {
        EMPTY_STR.to_string()
    }

    fn get_season_serialization_text(&self) -> &str {
        "Serialization"
    }

    fn get_episodes_chapters_text(&self) -> &str {
        "Chapters"
    }

    fn get_duration_volumes_text(&self) -> &str {
        "Volumes"
    }

    fn get_studios_staff_text(&self) -> &str {
        "Staff"
    }
}
