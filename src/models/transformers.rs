use std::collections::HashMap;

use crate::{
    models::{
        anilist_common::{CoverImage, Tag, TitleVariant},
        user_media_list::MediaListData,
    },
    utils::{formatter::*, statics::EMPTY_STR},
};

use html2md::parse_html;
use serenity::all::{CreateEmbed, CreateEmbedFooter};

pub trait Transformers {
    fn get_id(&self) -> u32;
    fn get_type(&self) -> &str;
    fn is_adult(&self) -> bool;
    fn get_mal_id(&self) -> Option<u32>;
    fn get_english_title(&self) -> Option<&str>;
    fn get_romaji_title(&self) -> Option<&str>;
    fn get_native_title(&self) -> Option<&str>;
    fn get_synonyms(&self) -> Option<&[String]>;
    fn get_format(&self) -> Option<&str>;
    fn get_status(&self) -> Option<&str>;
    fn get_genres(&self) -> &[String];
    fn get_source(&self) -> Option<&str>;
    fn get_cover_image(&self) -> &CoverImage;
    fn get_average_score(&self) -> Option<u32>;
    fn get_site_url(&self) -> &str;
    fn get_description(&self) -> Option<&str>;
    fn get_tags(&self) -> &[Tag];

    fn transform_mal_id(&self) -> Option<String>;
    fn transform_season_serialization(&self) -> String;
    fn transform_episodes_chapters(&self) -> String;
    fn transform_duration_volumes(&self) -> String;
    fn transform_studios_staff(&self) -> String;
    fn transform_links(&self) -> String;
    fn transform_trailer(&self) -> String;

    fn get_season_serialization_text(&self) -> &str;
    fn get_episodes_chapters_text(&self) -> &str;
    fn get_duration_volumes_text(&self) -> &str;
    fn get_studios_staff_text(&self) -> &str;

    fn transform_english_title(&self) -> String {
        let return_title = self
            .get_english_title()
            .or_else(|| self.get_romaji_title())
            .or_else(|| self.get_native_title())
            .unwrap_or_default();
        titlecase(return_title)
    }
    fn transform_romaji_title(&self) -> String {
        let return_title = self
            .get_romaji_title()
            .or_else(|| self.get_english_title())
            .or_else(|| self.get_native_title())
            .unwrap_or_default();
        titlecase(return_title)
    }
    fn transform_native_title(&self) -> String {
        // Native titles are typically Japanese, so titlecase would be wrong.
        // Fall back to romaji → english when native is missing.
        match self.get_native_title() {
            Some(title) => title.to_string(),
            None => match self.get_romaji_title() {
                Some(title) => titlecase(title),
                None => titlecase(self.get_english_title().unwrap_or_default()),
            },
        }
    }

    fn transform_format(&self) -> String {
        match self.get_format() {
            Some(format) => remove_underscores_and_titlecase(format),
            None => EMPTY_STR.to_string(),
        }
    }
    fn transform_status(&self) -> String {
        match self.get_status() {
            Some(status) => remove_underscores_and_titlecase(status),
            None => EMPTY_STR.to_string(),
        }
    }

    fn transform_genres(&self) -> String {
        let genres = self
            .get_genres()
            .iter()
            .map(|genre| code(titlecase(genre)))
            .collect::<Vec<String>>()
            .join(" - ");

        if genres.is_empty() {
            EMPTY_STR.to_string()
        } else {
            genres
        }
    }

    fn transform_source(&self) -> String {
        match self.get_source() {
            Some(source) => remove_underscores_and_titlecase(source),
            None => EMPTY_STR.to_string(),
        }
    }

    // CoverImage Transformers
    fn transform_color(&self) -> i32 {
        match self.get_cover_image().color.as_deref() {
            None => 0x0000ff,
            Some(color) => {
                i32::from_str_radix(color.trim_start_matches('#'), 16).unwrap_or(0x0000ff)
            }
        }
    }
    fn transform_thumbnail(&self) -> String {
        let cover = self.get_cover_image();
        cover
            .extra_large
            .as_deref()
            .or(cover.large.as_deref())
            .or(cover.medium.as_deref())
            .unwrap_or_default()
            .to_string()
    }

    fn transform_score(&self) -> String {
        match self.get_average_score() {
            Some(score) => format!("{score}/100"),
            None => EMPTY_STR.to_string(),
        }
    }

    fn transform_anilist(&self) -> String {
        self.get_site_url().to_string()
    }

    fn transform_description_and_mal_link(&self) -> String {
        let description = parse_html(self.get_description().unwrap_or("<i>No Description Yet<i>"));

        let url = self.transform_mal_id();

        match url {
            Some(link) => format!(
                "{description}\n\n**{}**",
                linker("MyAnimeList".to_string(), link),
            ),
            None => description,
        }
    }

    fn transform_tags(&self) -> String {
        let tags_list = self.get_tags();

        match tags_list.first() {
            Some(tag) => italics(tag.name.clone()),
            None => EMPTY_STR.to_string(),
        }
    }

    fn transform_response_embed(
        &self,
        guild_members_data: Option<HashMap<i64, MediaListData>>,
        title_variant: Option<TitleVariant>,
    ) -> CreateEmbed {
        let is_anime = self.get_type() == "anime";

        // Surface whichever variant the user typed as the primary title;
        // park the other one in the footer. Default (no signal) keeps the
        // long-standing Romaji-as-title behaviour.
        let (primary_title, footer_title) = match title_variant {
            Some(TitleVariant::English) => (
                self.transform_english_title(),
                self.transform_romaji_title(),
            ),
            // Native primary pairs with Romaji in the footer: Romaji is the
            // direct transliteration, so it acts as a pronunciation aid for
            // the Native script. English is intentionally omitted here — it
            // is the variant least related to a native-script search.
            Some(TitleVariant::Native) => {
                (self.transform_native_title(), self.transform_romaji_title())
            }
            Some(TitleVariant::Romaji) | None => (
                self.transform_romaji_title(),
                self.transform_english_title(),
            ),
        };

        let mut embed = CreateEmbed::new()
            // General Embed Fields
            .color(self.transform_color())
            .title(primary_title)
            .description(self.transform_description_and_mal_link())
            .url(self.transform_anilist())
            .thumbnail(self.transform_thumbnail())
            .footer(CreateEmbedFooter::new(footer_title))
            // self Data Fields
            // First line after MAL link
            .fields(vec![
                ("Type", titlecase(self.get_type()), true),
                ("Status", self.transform_status(), true),
                (
                    self.get_season_serialization_text(),
                    self.transform_season_serialization(),
                    true,
                ),
            ])
            // Second line after MAL link
            .fields(vec![
                ("Format", self.transform_format(), true),
                (
                    self.get_episodes_chapters_text(),
                    self.transform_episodes_chapters(),
                    true,
                ),
                (
                    self.get_duration_volumes_text(),
                    self.transform_duration_volumes(),
                    true,
                ),
            ])
            // Third line after MAL link
            .fields(vec![
                ("Source", self.transform_source(), true),       // Field 6
                ("Average Score", self.transform_score(), true), // Field 7
                // ("\u{200b}", &"\u{200b}".to_string(), true), // Would add a blank field
                ("Top Tag", self.transform_tags(), true), // Field 8
            ])
            // Fourth line after MAL link
            .fields(vec![("Genres", self.transform_genres(), false)])
            // Fifth line after MAL
            .field(
                self.get_studios_staff_text(),
                self.transform_studios_staff(),
                false,
            );

        // Sixth line after MAL link (Only for Anime response)
        if is_anime {
            embed = embed.fields(vec![
                ("Streaming", self.transform_links(), true), // Field 11
                ("Trailer", self.transform_trailer(), true), // Field 12
            ]);
        }

        // Build the scores field and return the embed
        match guild_members_data {
            Some(guild_members_data) => {
                let mut guild_members_data_string = String::default();
                for (user_id, score) in guild_members_data {
                    let current_member_data = score.format_for_embed(is_anime);
                    guild_members_data_string
                        .push_str(&format!("<@{user_id}>: {current_member_data}\n"));
                }
                embed.field("Guild Members", &guild_members_data_string, false)
            }
            None => embed,
        }
    }
}
