use serde::Deserialize;
use serenity::all::CreateEmbed;

use crate::utils::formatter::linker;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Studio {
    #[allow(dead_code)]
    id: u32,
    name: String,
    #[allow(dead_code)]
    is_animation_studio: bool,
    favourites: Option<u32>,
    site_url: String,
    media: Option<StudioMediaConnection>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StudioMediaConnection {
    nodes: Option<Vec<StudioMedia>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudioMedia {
    #[allow(dead_code)]
    id: u32,
    title: StudioMediaTitle,
    site_url: Option<String>,
    is_adult: Option<bool>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StudioMediaTitle {
    romaji: Option<String>,
    english: Option<String>,
}

impl Studio {
    pub fn transform_response_embed(&self) -> CreateEmbed {
        let mut embed = CreateEmbed::new()
            .color(0x00_68_A8)
            .title(&self.name)
            .url(&self.site_url);

        if let Some(favourites) = self.favourites {
            embed = embed.field("Favourites", favourites.to_string(), true);
        }

        let notable_anime = self.transform_media();
        if !notable_anime.is_empty() {
            embed = embed.field("Notable Anime", notable_anime, false);
        }

        embed
    }

    fn transform_media(&self) -> String {
        self.media
            .as_ref()
            .and_then(|media| media.nodes.as_deref())
            .unwrap_or_default()
            .iter()
            .filter(|media| !media.is_adult.unwrap_or(false))
            .filter_map(|media| {
                let title = media
                    .title
                    .english
                    .as_deref()
                    .or(media.title.romaji.as_deref())?;
                Some(match media.site_url.as_deref() {
                    Some(url) => format!("• {}", linker(title, url)),
                    None => format!("• {title}"),
                })
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_links_studio_and_shapes_available_fields() {
        let studio: Studio = serde_json::from_value(serde_json::json!({
            "id": 2,
            "name": "Kyoto Animation",
            "isAnimationStudio": true,
            "favourites": 1234,
            "siteUrl": "https://anilist.co/studio/2",
            "media": {
                "nodes": [{
                    "id": 20912,
                    "title": { "romaji": "Koe no Katachi", "english": "A Silent Voice" },
                    "siteUrl": "https://anilist.co/anime/20912",
                    "isAdult": false
                }]
            }
        }))
        .expect("studio fixture should deserialize");

        let embed = studio.transform_response_embed();
        let value = serde_json::to_value(embed).expect("embed should serialize");

        assert_eq!(value["title"], "Kyoto Animation");
        assert_eq!(value["url"], "https://anilist.co/studio/2");
        assert_eq!(value["fields"][0]["name"], "Favourites");
        assert_eq!(value["fields"][0]["value"], "1234");
        assert_eq!(value["fields"][1]["name"], "Notable Anime");
        assert!(
            value["fields"][1]["value"]
                .as_str()
                .unwrap()
                .contains("[A Silent Voice](https://anilist.co/anime/20912)")
        );
    }
}
