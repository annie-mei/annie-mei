use crate::utils::{
    formatter::{code, linker, titlecase},
    statics::EMPTY_STR,
};

use html2md::parse_html;
use serde::Deserialize;
use serenity::all::{CreateEmbed, CreateEmbedFooter};

const DISCORD_EMBED_DESCRIPTION_LIMIT: usize = 4096;
const DESCRIPTION_ELLIPSIS: &str = "...";
const MARKDOWN_SPOILER_CLASS: &str = "markdown_spoiler";
const SPAN_OPEN: &str = "<span";
const SPAN_CLOSE: &str = "</span>";

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    #[allow(dead_code)]
    id: u32,
    name: CharacterName,
    image: Option<CharacterImage>,
    description: Option<String>,
    gender: Option<String>,
    date_of_birth: Option<CharacterDate>,
    age: Option<String>,
    blood_type: Option<String>,
    favourites: Option<u32>,
    site_url: String,
    media: Option<CharacterMediaConnection>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CharacterName {
    pub full: Option<String>,
    pub native: Option<String>,
    pub alternative: Option<Vec<String>>,
    pub alternative_spoiler: Option<Vec<String>>,
    pub user_preferred: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CharacterImage {
    pub large: Option<String>,
    pub medium: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CharacterDate {
    year: Option<u32>,
    month: Option<u32>,
    day: Option<u32>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CharacterMediaConnection {
    nodes: Option<Vec<CharacterMedia>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CharacterMedia {
    #[allow(dead_code)]
    id: u32,
    #[serde(rename = "type")]
    media_type: Option<String>,
    title: CharacterMediaTitle,
    site_url: Option<String>,
    is_adult: Option<bool>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CharacterMediaTitle {
    romaji: Option<String>,
    english: Option<String>,
}

impl CharacterName {
    pub fn search_name(&self) -> String {
        self.user_preferred
            .as_deref()
            .or(self.full.as_deref())
            .or(self.native.as_deref())
            .unwrap_or_default()
            .to_string()
    }

    pub fn search_aliases(&self, allow_spoilers: bool) -> Vec<String> {
        let mut aliases = Vec::new();

        if let Some(full) = &self.full {
            aliases.push(full.clone());
        }

        if let Some(native) = &self.native {
            aliases.push(native.clone());
        }

        if let Some(alternative) = &self.alternative {
            aliases.extend(alternative.clone());
        }

        if allow_spoilers && let Some(alternative_spoiler) = &self.alternative_spoiler {
            aliases.extend(alternative_spoiler.clone());
        }

        aliases
    }

    pub fn has_alias(&self, alias: &str, allow_spoilers: bool) -> bool {
        self.search_aliases(allow_spoilers)
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(alias))
    }

    pub fn has_spoiler_alias(&self, alias: &str) -> bool {
        self.alternative_spoiler.as_ref().is_some_and(|aliases| {
            aliases
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(alias))
        })
    }
}

impl Character {
    pub fn name(&self) -> &CharacterName {
        &self.name
    }

    pub fn transform_name(&self) -> String {
        self.name
            .user_preferred
            .as_deref()
            .or(self.name.full.as_deref())
            .or(self.name.native.as_deref())
            .map_or_else(|| EMPTY_STR.to_string(), titlecase)
    }

    pub fn transform_footer_name(&self) -> String {
        self.name
            .native
            .as_deref()
            .filter(|native| *native != self.transform_name())
            .map(ToString::to_string)
            .or_else(|| {
                self.name
                    .alternative
                    .as_ref()
                    .and_then(|alternative| alternative.first())
                    .map(|name| titlecase(name))
            })
            .unwrap_or_else(|| EMPTY_STR.to_string())
    }

    pub fn transform_thumbnail(&self) -> Option<String> {
        self.image
            .as_ref()
            .and_then(|image| image.medium.as_deref().or(image.large.as_deref()))
            .filter(|url| !url.trim().is_empty())
            .map(ToString::to_string)
    }

    pub fn transform_description(&self, allow_spoilers: bool) -> String {
        let description_html = self
            .description
            .as_deref()
            .unwrap_or("<i>No Description Yet</i>");
        let filtered_description = if allow_spoilers {
            description_html.to_string()
        } else {
            strip_spoiler_html(description_html)
        };
        let description = parse_html(&filtered_description);

        if description.chars().count() <= DISCORD_EMBED_DESCRIPTION_LIMIT {
            return description;
        }

        description
            .chars()
            .take(DISCORD_EMBED_DESCRIPTION_LIMIT - DESCRIPTION_ELLIPSIS.len())
            .chain(DESCRIPTION_ELLIPSIS.chars())
            .collect()
    }

    pub fn transform_gender(&self) -> String {
        self.gender
            .as_deref()
            .map_or_else(|| EMPTY_STR.to_string(), titlecase)
    }

    pub fn transform_birthday(&self) -> String {
        let Some(date) = &self.date_of_birth else {
            return EMPTY_STR.to_string();
        };

        match (date.year, date.month, date.day) {
            (Some(year), Some(month), Some(day)) => format!("{year:04}-{month:02}-{day:02}"),
            (None, Some(month), Some(day)) => format!("{month:02}-{day:02}"),
            (Some(year), Some(month), None) => format!("{year:04}-{month:02}"),
            (Some(year), None, None) => year.to_string(),
            _ => EMPTY_STR.to_string(),
        }
    }

    pub fn transform_age(&self) -> String {
        self.age.clone().unwrap_or_else(|| EMPTY_STR.to_string())
    }

    pub fn transform_blood_type(&self) -> String {
        self.blood_type
            .clone()
            .unwrap_or_else(|| EMPTY_STR.to_string())
    }

    pub fn transform_favourites(&self) -> String {
        self.favourites.map_or_else(
            || EMPTY_STR.to_string(),
            |favourites| favourites.to_string(),
        )
    }

    pub fn media_is_all_adult(&self) -> bool {
        let Some(nodes) = self.media.as_ref().and_then(|media| media.nodes.as_ref()) else {
            return false;
        };

        !nodes.is_empty() && nodes.iter().all(|media| media.is_adult.unwrap_or(false))
    }

    pub fn has_adult_media(&self) -> bool {
        self.media
            .as_ref()
            .and_then(|media| media.nodes.as_ref())
            .is_some_and(|nodes| nodes.iter().any(|media| media.is_adult.unwrap_or(false)))
    }

    pub fn transform_media(&self, allow_adult: bool) -> String {
        let Some(media) = &self.media else {
            return EMPTY_STR.to_string();
        };
        let Some(nodes) = &media.nodes else {
            return EMPTY_STR.to_string();
        };

        let appearances = nodes
            .iter()
            .filter(|media| allow_adult || !media.is_adult.unwrap_or(false))
            .filter_map(|media| {
                let title = media
                    .title
                    .english
                    .as_deref()
                    .or(media.title.romaji.as_deref())?;
                let formatted_title = titlecase(title);
                let media_type = media
                    .media_type
                    .as_deref()
                    .map_or_else(|| EMPTY_STR.to_string(), titlecase);

                match media.site_url.as_deref() {
                    Some(url) => Some(format!(
                        "{} {}",
                        code(&media_type),
                        linker(&formatted_title, url)
                    )),
                    None => Some(format!("{} {}", code(&media_type), formatted_title)),
                }
            })
            .collect::<Vec<String>>()
            .join("\n");

        if appearances.is_empty() {
            EMPTY_STR.to_string()
        } else {
            appearances
        }
    }

    pub fn transform_response_embed(
        &self,
        allow_adult_media: bool,
        allow_spoilers: bool,
    ) -> CreateEmbed {
        let embed = CreateEmbed::new()
            .color(0x00_68_A8)
            .title(self.transform_name())
            .description(self.transform_description(allow_spoilers))
            .url(&self.site_url)
            .footer(CreateEmbedFooter::new(self.transform_footer_name()))
            .fields(vec![
                ("Gender", self.transform_gender(), true),
                ("Age", self.transform_age(), true),
                ("Birthday", self.transform_birthday(), true),
            ])
            .fields(vec![
                ("Blood Type", self.transform_blood_type(), true),
                ("Favourites", self.transform_favourites(), true),
            ])
            .field("Appears In", self.transform_media(allow_adult_media), false);

        match self.transform_thumbnail() {
            Some(thumbnail) => embed.thumbnail(thumbnail),
            None => embed,
        }
    }
}

fn strip_spoiler_html(html: &str) -> String {
    let mut output = String::default();
    let mut remaining = html;

    while let Some(class_index) = remaining.find(MARKDOWN_SPOILER_CLASS) {
        let Some(open_start) = remaining[..class_index].rfind(SPAN_OPEN) else {
            output.push_str(&remaining[..class_index + MARKDOWN_SPOILER_CLASS.len()]);
            remaining = &remaining[class_index + MARKDOWN_SPOILER_CLASS.len()..];
            continue;
        };

        output.push_str(&remaining[..open_start]);

        let Some(open_end_offset) = remaining[class_index..].find('>') else {
            break;
        };
        let mut cursor = class_index + open_end_offset + 1;
        let mut span_depth = 1;

        while span_depth > 0 {
            let next_open = remaining[cursor..]
                .find(SPAN_OPEN)
                .map(|offset| cursor + offset);
            let next_close = remaining[cursor..]
                .find(SPAN_CLOSE)
                .map(|offset| cursor + offset);

            match (next_open, next_close) {
                (Some(open), Some(close)) if open < close => {
                    span_depth += 1;
                    cursor = open + SPAN_OPEN.len();
                }
                (_, Some(close)) => {
                    span_depth -= 1;
                    cursor = close + SPAN_CLOSE.len();
                }
                _ => {
                    cursor = remaining.len();
                    break;
                }
            }
        }

        remaining = &remaining[cursor..];
    }

    output.push_str(remaining);
    output
}

#[cfg(test)]
mod tests {
    use super::{Character, DESCRIPTION_ELLIPSIS, DISCORD_EMBED_DESCRIPTION_LIMIT, EMPTY_STR};

    fn sample_character() -> Character {
        serde_json::from_value(serde_json::json!({
            "id": 40,
            "name": {
                "full": "Lelouch Lamperouge",
                "native": "ルルーシュ・ランペルージ",
                "alternative": ["Lelouch vi Britannia"],
                "alternativeSpoiler": ["Lelouch vi Britannia the 99th Emperor"],
                "userPreferred": "Lelouch Lamperouge"
            },
            "image": {
                "large": "https://example.com/large.jpg",
                "medium": "https://example.com/medium.jpg"
            },
            "description": "<p>A former prince.</p>",
            "gender": "Male",
            "dateOfBirth": { "year": null, "month": 12, "day": 5 },
            "age": "17",
            "bloodType": "A",
            "favourites": 1000,
            "siteUrl": "https://anilist.co/character/40",
            "media": {
                "nodes": [{
                    "id": 1575,
                    "type": "ANIME",
                    "title": {
                        "romaji": "Code Geass: Hangyaku no Lelouch",
                        "english": "Code Geass: Lelouch of the Rebellion"
                    },
                    "siteUrl": "https://anilist.co/anime/1575",
                    "isAdult": false
                }]
            }
        }))
        .expect("sample character JSON should deserialize")
    }

    #[test]
    fn transforms_name_from_user_preferred() {
        assert_eq!(sample_character().transform_name(), "Lelouch Lamperouge");
    }

    #[test]
    fn transforms_partial_birthday() {
        assert_eq!(sample_character().transform_birthday(), "12-05");
    }

    #[test]
    fn transforms_media_appearances() {
        let appearances = sample_character().transform_media(false);

        assert!(appearances.contains("Code Geass: Lelouch of the Rebellion"));
        assert!(appearances.contains("https://anilist.co/anime/1575"));
    }

    #[test]
    fn spoiler_aliases_require_explicit_allowance() {
        let character = sample_character();

        assert!(
            !character
                .name()
                .has_alias("Lelouch vi Britannia the 99th Emperor", false)
        );
        assert!(
            character
                .name()
                .has_alias("Lelouch vi Britannia the 99th Emperor", true)
        );
    }

    #[test]
    fn media_is_all_adult_requires_only_adult_appearances() {
        let character: Character = serde_json::from_value(serde_json::json!({
            "id": 1,
            "name": {
                "full": "Adult Character",
                "native": null,
                "alternative": [],
                "userPreferred": "Adult Character"
            },
            "image": null,
            "description": null,
            "gender": null,
            "dateOfBirth": null,
            "age": null,
            "bloodType": null,
            "favourites": null,
            "siteUrl": "https://anilist.co/character/1",
            "media": {
                "nodes": [{
                    "id": 1,
                    "type": "MANGA",
                    "title": { "romaji": "Adult Manga", "english": null },
                    "siteUrl": "https://anilist.co/manga/1",
                    "isAdult": true
                }]
            }
        }))
        .expect("sample character JSON should deserialize");

        assert!(character.media_is_all_adult());
    }

    #[test]
    fn transform_media_includes_adult_appearances_when_allowed() {
        let character: Character = serde_json::from_value(serde_json::json!({
            "id": 1,
            "name": {
                "full": "Adult Character",
                "native": null,
                "alternative": [],
                "userPreferred": "Adult Character"
            },
            "image": null,
            "description": null,
            "gender": null,
            "dateOfBirth": null,
            "age": null,
            "bloodType": null,
            "favourites": null,
            "siteUrl": "https://anilist.co/character/1",
            "media": {
                "nodes": [{
                    "id": 1,
                    "type": "MANGA",
                    "title": { "romaji": "Adult Manga", "english": null },
                    "siteUrl": "https://anilist.co/manga/1",
                    "isAdult": true
                }]
            }
        }))
        .expect("sample character JSON should deserialize");

        assert_eq!(character.transform_media(false), EMPTY_STR);
        assert!(character.transform_media(true).contains("Adult Manga"));
    }

    #[test]
    fn transform_description_respects_discord_embed_limit() {
        let character: Character = serde_json::from_value(serde_json::json!({
            "id": 1,
            "name": {
                "full": "Long Bio",
                "native": null,
                "alternative": [],
                "userPreferred": "Long Bio"
            },
            "image": null,
            "description": "a".repeat(5000),
            "gender": null,
            "dateOfBirth": null,
            "age": null,
            "bloodType": null,
            "favourites": null,
            "siteUrl": "https://anilist.co/character/1",
            "media": { "nodes": [] }
        }))
        .expect("sample character JSON should deserialize");

        let description = character.transform_description(true);

        assert_eq!(description.chars().count(), DISCORD_EMBED_DESCRIPTION_LIMIT);
        assert!(description.ends_with(DESCRIPTION_ELLIPSIS));
    }

    #[test]
    fn transform_description_filters_spoiler_html_when_disallowed() {
        let character: Character = serde_json::from_value(serde_json::json!({
            "id": 650,
            "name": {
                "full": "Lust",
                "native": null,
                "alternative": [],
                "alternativeSpoiler": [],
                "userPreferred": "Lust"
            },
            "image": null,
            "description": "<p>Visible text.</p><p><span class='markdown_spoiler'><span>Hidden spoiler.</span></span></p>",
            "gender": null,
            "dateOfBirth": null,
            "age": null,
            "bloodType": null,
            "favourites": null,
            "siteUrl": "https://anilist.co/character/650",
            "media": { "nodes": [] }
        }))
        .expect("sample character JSON should deserialize");

        let disallowed = character.transform_description(false);
        let allowed = character.transform_description(true);

        assert!(disallowed.contains("Visible text."));
        assert!(!disallowed.contains("Hidden spoiler."));
        assert!(allowed.contains("Hidden spoiler."));
    }

    #[test]
    fn transform_thumbnail_ignores_missing_image_urls() {
        let character: Character = serde_json::from_value(serde_json::json!({
            "id": 1,
            "name": {
                "full": "No Image",
                "native": null,
                "alternative": [],
                "userPreferred": "No Image"
            },
            "image": { "large": "", "medium": null },
            "description": null,
            "gender": null,
            "dateOfBirth": null,
            "age": null,
            "bloodType": null,
            "favourites": null,
            "siteUrl": "https://anilist.co/character/1",
            "media": { "nodes": [] }
        }))
        .expect("sample character JSON should deserialize");

        let embed = character.transform_response_embed(false, false);
        let value = serde_json::to_value(&embed).expect("embed serializes");

        assert!(character.transform_thumbnail().is_none());
        assert!(value.get("thumbnail").is_none());
    }

    #[test]
    fn success_embed_serializes() {
        let embed = sample_character().transform_response_embed(false, false);
        let value = serde_json::to_value(&embed).expect("embed serializes");

        assert_eq!(value["title"], "Lelouch Lamperouge");
        assert_eq!(value["url"], "https://anilist.co/character/40");
        assert_eq!(value["thumbnail"]["url"], "https://example.com/medium.jpg");
    }
}
