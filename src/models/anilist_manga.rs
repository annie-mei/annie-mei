use crate::{
    models::{
        anilist_common::{CoverImage, Tag, Title},
        transformers::Transformers,
    },
    utils::{
        formatter::code,
        statics::{ANILIST_STATUS_RELEASING, EMPTY_STR},
    },
};

use chrono::NaiveDate;
use serde::Deserialize;
use titlecase::titlecase;
use tracing::instrument;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Manga {
    #[serde(rename = "type")]
    media_type: Option<String>,
    #[allow(dead_code)]
    id: u32,
    id_mal: Option<u32>,
    is_adult: Option<bool>,
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
    pub role: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Nodes {
    pub name: StaffName,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StaffName {
    pub full: String,
}

impl Manga {
    /// Renders serialization dates, validating each endpoint independently.
    /// Missing or invalid endpoints are hidden instead of panicking, and the
    /// `EMPTY_STR` sentinel is returned when neither endpoint is usable.
    #[instrument(name = "anilist_manga.transform_date", skip(self))]
    pub fn transform_date(&self) -> String {
        let formatted_start_date = self.start_date.as_ref().and_then(get_formatted_date_string);
        let formatted_end_date = self.end_date.as_ref().and_then(get_formatted_date_string);

        match (formatted_start_date, formatted_end_date) {
            (Some(start), Some(end)) => format!("{start} - {end}"),
            (Some(start), None) => start,
            (None, Some(end)) => end,
            (None, None) => EMPTY_STR.to_string(),
        }
    }

    pub fn transform_chapters(&self) -> String {
        match &self.chapters {
            Some(chapters) => {
                if self.status.as_deref() == Some(ANILIST_STATUS_RELEASING) {
                    format!("{chapters} written")
                } else {
                    chapters.to_string()
                }
            }
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
        let Some(staff) = self.staff.as_ref() else {
            return EMPTY_STR.to_string();
        };

        if staff.edges.is_empty() || staff.nodes.is_empty() {
            return EMPTY_STR.to_string();
        }

        let mut mangaka_index = 0_usize;
        let mut artist_index = 0_usize;

        for (index, edge) in staff.edges.iter().enumerate() {
            let role_lower = edge.role.to_lowercase();
            if role_lower.contains("story") {
                mangaka_index = index;
            }
            if role_lower.contains("art") {
                artist_index = index;
            }
        }

        // Guard against AniList returning mismatched edges/nodes arrays:
        // use `.get()` so an out-of-range role index falls back to the
        // `EMPTY_STR` sentinel instead of panicking on direct indexing.
        let mangaka_name = staff
            .nodes
            .get(mangaka_index)
            .map(|node| node.name.full.as_str());
        let artist_name = staff
            .nodes
            .get(artist_index)
            .map(|node| node.name.full.as_str());

        match (mangaka_name, artist_name) {
            (Some(m), Some(a)) if m == a => code(&titlecase(m)),
            (Some(m), Some(a)) => format!("{} x {}", code(&titlecase(m)), code(&titlecase(a))),
            (Some(name), None) | (None, Some(name)) => code(&titlecase(name)),
            (None, None) => EMPTY_STR.to_string(),
        }
    }
}

/// Formats a single AniList date endpoint, returning `None` when the date is
/// invalid rather than inventing missing precision. A year is required and a
/// day without a month is invalid; valid dates keep the existing formats
/// (`%b %e %Y`, `%b %Y`, and `%Y`).
#[instrument(name = "anilist_manga.get_formatted_date_string", skip(date))]
fn get_formatted_date_string(date: &AnilistDate) -> Option<String> {
    let year = date.year?;
    // AniList years arrive as `u32`, but Chrono only supports years that fit
    // `i32`, so out-of-range values are invalid rather than a panic.
    let year = i32::try_from(year).ok()?;

    match (date.month, date.day) {
        (Some(month), Some(day)) => NaiveDate::from_ymd_opt(year, month, day)
            .map(|date_string| date_string.format("%b %e %Y").to_string()),
        (Some(month), None) => NaiveDate::from_ymd_opt(year, month, 1)
            .map(|date_string| date_string.format("%b %Y").to_string()),
        (None, None) => NaiveDate::from_ymd_opt(year, 1, 1)
            .map(|date_string| date_string.format("%Y").to_string()),
        // A day without a month is invalid.
        (None, Some(_)) => None,
    }
}

impl Transformers for Manga {
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

#[cfg(test)]
mod tests {
    use super::Manga;
    use crate::utils::statics::{ANILIST_STATUS_FINISHED, ANILIST_STATUS_RELEASING, EMPTY_STR};
    use serde_json::json;

    fn sample_manga(status: &str, chapters: Option<u32>) -> Manga {
        sample_manga_with_dates(status, chapters, json!(null), json!(null))
    }

    fn sample_manga_with_dates(
        status: &str,
        chapters: Option<u32>,
        start_date: serde_json::Value,
        end_date: serde_json::Value,
    ) -> Manga {
        serde_json::from_value(json!({
            "type": "MANGA",
            "id": 1,
            "idMal": null,
            "isAdult": false,
            "title": {
                "romaji": "Sample",
                "english": "Sample",
                "native": "サンプル"
            },
            "synonyms": null,
            "startDate": start_date,
            "endDate": end_date,
            "format": null,
            "status": status,
            "chapters": chapters,
            "volumes": null,
            "genres": [],
            "source": null,
            "coverImage": {
                "extraLarge": null,
                "large": null,
                "medium": "https://example.com/image.jpg",
                "color": null
            },
            "averageScore": null,
            "staff": null,
            "siteUrl": "https://anilist.co/manga/1",
            "description": null,
            "tags": []
        }))
        .expect("sample manga JSON should deserialize")
    }

    #[test]
    fn transform_chapters_uses_written_suffix_for_releasing_manga() {
        let manga = sample_manga(ANILIST_STATUS_RELEASING, Some(110));

        assert_eq!(manga.transform_chapters(), "110 written");
    }

    #[test]
    fn transform_chapters_uses_plain_count_for_non_releasing_manga() {
        let manga = sample_manga(ANILIST_STATUS_FINISHED, Some(110));

        assert_eq!(manga.transform_chapters(), "110");
    }

    fn date_json(year: Option<u32>, month: Option<u32>, day: Option<u32>) -> serde_json::Value {
        json!({ "year": year, "month": month, "day": day })
    }

    #[test]
    fn transform_date_returns_sentinel_when_dates_absent() {
        let manga = sample_manga(ANILIST_STATUS_FINISHED, Some(10));

        assert_eq!(manga.transform_date(), EMPTY_STR);
    }

    #[test]
    fn transform_date_returns_sentinel_when_dates_all_null() {
        let manga = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(None, None, None),
            date_json(None, None, None),
        );

        assert_eq!(manga.transform_date(), EMPTY_STR);
    }

    #[test]
    fn transform_date_formats_year_only_start() {
        let manga = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(Some(2024), None, None),
            json!(null),
        );

        assert_eq!(manga.transform_date(), "2024");
    }

    #[test]
    fn transform_date_formats_year_and_month() {
        let manga = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(Some(2024), Some(3), None),
            json!(null),
        );

        assert_eq!(manga.transform_date(), "Mar 2024");
    }

    #[test]
    fn transform_date_formats_full_date() {
        let manga = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(Some(2024), Some(3), Some(5)),
            json!(null),
        );

        assert_eq!(manga.transform_date(), "Mar  5 2024");
    }

    #[test]
    fn transform_date_returns_sentinel_for_month_without_year() {
        let manga = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(None, Some(3), None),
            json!(null),
        );

        assert_eq!(manga.transform_date(), EMPTY_STR);
    }

    #[test]
    fn transform_date_returns_sentinel_for_day_without_month() {
        let manga = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(Some(2024), None, Some(5)),
            json!(null),
        );

        assert_eq!(manga.transform_date(), EMPTY_STR);
    }

    #[test]
    fn transform_date_formats_leap_day_in_leap_year() {
        let manga = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(Some(2024), Some(2), Some(29)),
            json!(null),
        );

        assert_eq!(manga.transform_date(), "Feb 29 2024");
    }

    #[test]
    fn transform_date_returns_sentinel_for_invalid_leap_day() {
        let manga = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(Some(2023), Some(2), Some(29)),
            json!(null),
        );

        assert_eq!(manga.transform_date(), EMPTY_STR);
    }

    #[test]
    fn transform_date_returns_sentinel_for_invalid_month_and_day() {
        let invalid_month = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(Some(2024), Some(13), Some(1)),
            json!(null),
        );
        let invalid_day = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(Some(2024), Some(6), Some(32)),
            json!(null),
        );

        assert_eq!(invalid_month.transform_date(), EMPTY_STR);
        assert_eq!(invalid_day.transform_date(), EMPTY_STR);
    }

    #[test]
    fn transform_date_returns_sentinel_for_unrepresentable_year() {
        let full_precision = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(Some(u32::MAX), Some(3), Some(5)),
            json!(null),
        );
        let year_only = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(Some(u32::MAX), None, None),
            json!(null),
        );

        assert_eq!(full_precision.transform_date(), EMPTY_STR);
        assert_eq!(year_only.transform_date(), EMPTY_STR);
    }

    #[test]
    fn transform_date_joins_valid_start_and_end() {
        let full_end = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(Some(2024), Some(3), Some(5)),
            date_json(Some(2025), Some(6), Some(15)),
        );
        let year_month_end = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(Some(2024), Some(3), Some(5)),
            date_json(Some(2025), Some(6), None),
        );
        let year_only_end = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(Some(2024), Some(3), Some(5)),
            date_json(Some(2025), None, None),
        );

        assert_eq!(full_end.transform_date(), "Mar  5 2024 - Jun 15 2025");
        assert_eq!(year_month_end.transform_date(), "Mar  5 2024 - Jun 2025");
        assert_eq!(year_only_end.transform_date(), "Mar  5 2024 - 2025");
    }

    #[test]
    fn transform_date_shows_valid_start_when_end_invalid() {
        let missing_precision_end = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(Some(2024), Some(3), Some(5)),
            date_json(None, Some(3), None),
        );
        let invalid_calendar_end = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(Some(2024), Some(3), Some(5)),
            date_json(Some(2025), Some(2), Some(30)),
        );

        assert_eq!(missing_precision_end.transform_date(), "Mar  5 2024");
        assert_eq!(invalid_calendar_end.transform_date(), "Mar  5 2024");
    }

    #[test]
    fn transform_date_shows_valid_end_when_start_invalid() {
        let manga = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(None, Some(3), Some(5)),
            date_json(Some(2025), Some(6), Some(15)),
        );

        assert_eq!(manga.transform_date(), "Jun 15 2025");
    }

    #[test]
    fn transform_date_returns_sentinel_when_neither_endpoint_valid() {
        let manga = sample_manga_with_dates(
            ANILIST_STATUS_FINISHED,
            Some(10),
            date_json(None, Some(3), Some(5)),
            date_json(Some(2025), Some(2), Some(30)),
        );

        assert_eq!(manga.transform_date(), EMPTY_STR);
    }
}
