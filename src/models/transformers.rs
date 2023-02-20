use std::collections::HashMap;

use crate::{
    models::{
        anilist_common::{CoverImage, Tag},
        user_media_list::MediaListData,
    },
    utils::{formatter::*, statics::EMPTY_STR},
};

use html2md::parse_html;
use serenity::builder::CreateEmbed;

pub trait Transformers {
    fn get_id(&self) -> u32;
    fn get_type(&self) -> String;
    fn get_mal_id(&self) -> Option<u32>;
    fn get_english_title(&self) -> Option<String>;
    fn get_romaji_title(&self) -> Option<String>;
    fn get_native_title(&self) -> Option<String>;
    fn get_synonyms(&self) -> Option<Vec<String>>;
    fn get_format(&self) -> Option<String>;
    fn get_status(&self) -> Option<String>;
    fn get_genres(&self) -> Vec<String>;
    fn get_source(&self) -> Option<String>;
    fn get_cover_image(&self) -> CoverImage;
    fn get_average_score(&self) -> Option<u32>;
    fn get_site_url(&self) -> String;
    fn get_description(&self) -> Option<String>;
    fn get_tags(&self) -> Vec<Tag>;

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
        let english_title = self.get_english_title();
        let return_title = match english_title {
            Some(title) => title,
            None => match self.get_romaji_title() {
                Some(title) => title,
                None => self.get_native_title().unwrap_or_default(),
            },
        };
        titlecase(&return_title)
    }
    fn transform_romaji_title(&self) -> String {
        let romaji_title = self.get_romaji_title();
        let return_title = match romaji_title {
            Some(title) => title,
            None => match self.get_english_title() {
                Some(title) => title,
                None => self.get_native_title().unwrap_or_default(),
            },
        };
        titlecase(&return_title)
    }
    fn transform_native_title(&self) -> String {
        let native_title = self.get_native_title();
        let return_title = match native_title {
            Some(title) => title,
            None => match self.get_romaji_title() {
                Some(title) => title,
                None => self.get_english_title().unwrap_or_default(),
            },
        };
        titlecase(&return_title)
    }

    fn transform_format(&self) -> String {
        match self.get_format() {
            Some(format) => remove_underscores_and_titlecase(&format),
            None => EMPTY_STR.to_string(),
        }
    }
    fn transform_status(&self) -> String {
        match self.get_status() {
            Some(status) => remove_underscores_and_titlecase(&status),
            None => EMPTY_STR.to_string(),
        }
    }

    fn transform_genres(&self) -> String {
        let genres = self
            .get_genres()
            .into_iter()
            .map(|genre| code(titlecase(&genre)))
            .collect::<Vec<String>>();
        let genres = genres.join(" - ");

        if genres.is_empty() {
            EMPTY_STR.to_string()
        } else {
            genres
        }
    }

    fn transform_source(&self) -> String {
        match self.get_source() {
            Some(source) => remove_underscores_and_titlecase(&source),
            None => EMPTY_STR.to_string(),
        }
    }

    // CoverImage Transformers
    fn transform_color(&self) -> i32 {
        match self.get_cover_image().color.as_ref() {
            None => 0x0000ff,
            Some(color) => {
                i32::from_str_radix(color.trim_start_matches('#'), 16).unwrap_or(0x0000ff)
            }
        }
    }
    fn transform_thumbnail(&self) -> String {
        let extra_large = self.get_cover_image().extra_large;
        let large = self.get_cover_image().large;
        let medium = self.get_cover_image().medium;

        if let Some(value) = extra_large {
            return value;
        }

        if let Some(value) = large {
            return value;
        }

        medium.unwrap()
    }

    fn transform_score(&self) -> String {
        match self.get_average_score() {
            Some(score) => format!("{score}/100"),
            None => EMPTY_STR.to_string(),
        }
    }

    fn transform_anilist(&self) -> String {
        self.get_site_url()
    }

    fn transform_description_and_mal_link(&self) -> String {
        let description = parse_html(
            &self
                .get_description()
                .unwrap_or_else(|| "<i>No Description Yet<i>".to_string()),
        );

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

        if tags_list.is_empty() {
            EMPTY_STR.to_string()
        } else {
            italics(tags_list.first().unwrap().name.to_string())
        }
    }

    fn transform_response_embed(
        &self,
        guild_members_data: Option<HashMap<i64, MediaListData>>,
    ) -> CreateEmbed {
        let is_anime = self.get_type() == "anime";
        let mut embed = CreateEmbed::default();
        embed
            // General Embed Fields
            .color(self.transform_color())
            .title(self.transform_romaji_title())
            .description(self.transform_description_and_mal_link())
            .url(self.transform_anilist())
            .thumbnail(self.transform_thumbnail())
            .footer(|f| f.text(self.transform_english_title()))
            // self Data Fields
            // First line after MAL link
            .fields(vec![
                ("Type", titlecase(&self.get_type()), true),
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
            embed.fields(vec![
                ("Streaming", self.transform_links(), true), // Field 11
                ("Trailer", self.transform_trailer(), true), // Field 12
            ]);
        }

        // Build the scores field and return the embed
        let embed = match guild_members_data {
            Some(guild_members_data) => {
                let mut guild_members_data_string = String::default();
                for (user_id, score) in guild_members_data {
                    let current_member_data = score.format_for_embed(is_anime);
                    guild_members_data_string
                        .push_str(&format!("<@{user_id}>: {current_member_data}\n"));
                }
                embed.field("Guild Members", &guild_members_data_string, false)
            }
            None => &mut embed,
        };
        embed.clone()
    }
}
