use crate::{
    commands::{
        input_validation::validate_search_term,
        recommend::queries::{fetch_recommendations_by_id, fetch_recommendations_by_search},
        response::CommandResponse,
    },
    models::{
        anilist_common::TitleVariant,
        anilist_recommendation::{
            Recommendation, RecommendationMedia, RecommendationMediaResponse, RecommendedMedia,
        },
        media_response::FetchResponse as SearchResponse,
        media_type::MediaType,
        transformers::Transformers,
    },
    utils::{
        channel::is_nsfw_channel,
        fetch_by_arguments::{fetch_by_id, fetch_by_name},
        formatter::{code, linker, remove_underscores_and_titlecase, titlecase},
        privacy::configure_sentry_scope,
        redis::{check_cache, try_to_cache_response},
        statics::{EMPTY_STR, NOT_FOUND_ANIME, NOT_FOUND_MANGA, NSFW_NOT_ALLOWED},
    },
};

use redis::RedisResult;
use serde_json::json;
use serenity::{
    all::{
        CommandDataOption, CommandDataOptionValue, CommandInteraction, CreateCommandOption,
        CreateEmbed, CreateEmbedFooter, EditInteractionResponse,
    },
    builder::CreateCommand,
    client::Context,
    model::application::CommandOptionType,
};
use tokio::task;
use tracing::{error, info, instrument};

const TYPE_OPTION: &str = "type";
const SEARCH_OPTION: &str = "search";
const ANIME_TYPE: &str = "anime";
const MANGA_TYPE: &str = "manga";
const RECOMMENDATION_LIMIT: usize = 5;

pub fn register() -> CreateCommand {
    CreateCommand::new("recommend")
        .description("Fetch AniList recommendations for an anime or manga")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                TYPE_OPTION,
                "Whether to find anime or manga recommendations",
            )
            .add_string_choice("Anime", ANIME_TYPE)
            .add_string_choice("Manga", MANGA_TYPE)
            .required(true),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                SEARCH_OPTION,
                "AniList ID or search term",
            )
            .required(true),
        )
}

#[instrument(name = "command.recommend.parse_options", skip(options))]
fn parse_recommend_options(options: &[CommandDataOption]) -> Option<(MediaType, String)> {
    let media_type = options
        .iter()
        .find(|option| option.name == TYPE_OPTION)
        .and_then(|option| match &option.value {
            CommandDataOptionValue::String(value) => match value.as_str() {
                ANIME_TYPE => Some(MediaType::Anime),
                MANGA_TYPE => Some(MediaType::Manga),
                _ => None,
            },
            _ => None,
        })?;

    let search_term = options
        .iter()
        .find(|option| option.name == SEARCH_OPTION)
        .and_then(|option| match &option.value {
            CommandDataOptionValue::String(search_term) => Some(search_term.clone()),
            _ => None,
        })?;

    Some((media_type, search_term))
}

pub fn handle_recommend(
    media: Option<RecommendationMedia>,
    media_type: MediaType,
    title_variant: Option<TitleVariant>,
    allow_adult_media: bool,
) -> CommandResponse {
    let Some(media) = media else {
        return CommandResponse::Content(not_found_message(&media_type).to_string());
    };

    if media.is_adult() && !allow_adult_media {
        return CommandResponse::Content(NSFW_NOT_ALLOWED.to_string());
    }

    let recommendations = media
        .recommendations()
        .filter_map(|recommendation| {
            let recommended_media = recommendation.recommended_media()?;
            if recommended_media.is_adult() && !allow_adult_media {
                return None;
            }
            Some((recommendation, recommended_media))
        })
        .take(RECOMMENDATION_LIMIT)
        .collect::<Vec<_>>();

    if recommendations.is_empty() {
        return CommandResponse::Content(format!(
            "No recommendations found for {}.",
            media_title(&media, title_variant)
        ));
    }

    CommandResponse::Embed(Box::new(recommendations_embed(
        &media,
        &recommendations,
        title_variant,
    )))
}

#[instrument(name = "command.recommend.run", skip(ctx, interaction))]
pub async fn run(ctx: &Context, interaction: &mut CommandInteraction) {
    let _ = interaction.defer(&ctx.http).await;

    let Some((media_type, search_term)) = parse_recommend_options(&interaction.data.options) else {
        let builder = EditInteractionResponse::new().content(
            "Missing or invalid options — choose a type and provide an anime or manga name or ID.",
        );
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    };

    if let Err(err) = validate_search_term(&search_term) {
        let builder = EditInteractionResponse::new().content(format!(
            "Invalid search input: {err}. Please check your input and try again."
        ));
        let _ = interaction.edit_response(&ctx.http, builder).await;
        return;
    }

    configure_sentry_scope(
        "Recommend",
        interaction.user.id.get(),
        Some(json!({
            "type": media_type.as_ref(),
            "search": search_term,
        })),
    );

    info!(
        media_type = ?media_type,
        "Got command 'recommend' with search_term: {search_term}"
    );

    let fetch_result = fetch_recommendation_media(&search_term, media_type.clone()).await;
    let (media, title_variant) = match fetch_result {
        Some((media, variant)) => (Some(media), Some(variant)),
        None => (None, None),
    };
    let allow_adult_media =
        is_nsfw_channel(ctx, interaction.channel_id, interaction.guild_id).await;
    let response = handle_recommend(media, media_type, title_variant, allow_adult_media);

    let _result = match response {
        CommandResponse::Content(text) | CommandResponse::Message(text) => {
            let builder = EditInteractionResponse::new().content(text);
            interaction.edit_response(&ctx.http, builder).await
        }
        CommandResponse::Embed(embed) => {
            let builder = EditInteractionResponse::new().embed(*embed);
            interaction.edit_response(&ctx.http, builder).await
        }
    };
}

#[instrument(name = "command.recommend.fetch", fields(media_type = ?media_type, search_len = search_term.len()))]
async fn fetch_recommendation_media(
    search_term: &str,
    media_type: MediaType,
) -> Option<(RecommendationMedia, TitleVariant)> {
    match search_term.parse::<u32>() {
        Ok(id) => fetch_recommendation_media_by_id(id, media_type).await,
        Err(_) => fetch_recommendation_media_by_search(search_term, media_type).await,
    }
}

#[instrument(name = "command.recommend.fetch_by_id", fields(media_type = ?media_type, id = id))]
async fn fetch_recommendation_media_by_id(
    id: u32,
    media_type: MediaType,
) -> Option<(RecommendationMedia, TitleVariant)> {
    let query = fetch_recommendations_by_id(anilist_type(&media_type));
    let fetched_data = match fetch_by_id(query, id).await {
        Ok(data) => data,
        Err(err) => {
            error!(error = %err, id = id, "Failed to fetch AniList recommendations by id");
            return None;
        }
    };
    let response: RecommendationMediaResponse = match serde_json::from_str(&fetched_data) {
        Ok(response) => response,
        Err(err) => {
            error!(error = %err, "Failed to deserialize AniList recommendation id response");
            return None;
        }
    };

    response
        .data
        .and_then(|data| data.media)
        .map(|media| (media, TitleVariant::Romaji))
}

#[instrument(name = "command.recommend.fetch_by_search", skip(search_term), fields(media_type = ?media_type, search_len = search_term.len()))]
async fn fetch_recommendation_media_by_search(
    search_term: &str,
    media_type: MediaType,
) -> Option<(RecommendationMedia, TitleVariant)> {
    let query = fetch_recommendations_by_search(anilist_type(&media_type));
    let cache_key = recommendation_cache_key(&media_type, search_term);
    let cache_key_for_lookup = cache_key.clone();

    let fetched_data = match task::spawn_blocking(move || {
        read_cached_recommendation_response(cache_key_for_lookup)
    })
    .await
    {
        Ok(Ok(cached_value)) => {
            info!("Cache hit for {:#?}", cache_key);
            cached_value
        }
        Ok(Err(err)) => {
            info!("Cache miss for {:#?} with error {:#?}", cache_key, err);
            fetch_recommendations_from_network_and_cache(query, search_term.to_string(), cache_key)
                .await?
        }
        Err(err) => {
            error!(error = %err, "Failed to read AniList recommendation cache");
            fetch_recommendations_from_network_and_cache(query, search_term.to_string(), cache_key)
                .await?
        }
    };
    let response: SearchResponse<RecommendationMedia> = match serde_json::from_str(&fetched_data) {
        Ok(response) => response,
        Err(err) => {
            error!(error = %err, "Failed to deserialize AniList recommendation search response");
            return None;
        }
    };

    response.fuzzy_match(search_term, media_type)
}

#[instrument(
    name = "command.recommend.fetch_from_network_and_cache",
    skip(query),
    fields(cache_key = %cache_key, lookup_len = search_term.len())
)]
async fn fetch_recommendations_from_network_and_cache(
    query: String,
    search_term: String,
    cache_key: String,
) -> Option<String> {
    let response = match fetch_by_name(query, search_term).await {
        Ok(data) => data,
        Err(err) => {
            error!(error = %err, "Failed to fetch AniList recommendations by search");
            return None;
        }
    };

    let cache_key_for_write = cache_key.clone();
    let response_to_cache = response.clone();
    if let Err(err) = task::spawn_blocking(move || {
        write_cached_recommendation_response(cache_key_for_write, response_to_cache)
    })
    .await
    {
        error!(error = %err, cache_key = %cache_key, "Failed to cache AniList recommendation response");
    }

    Some(response)
}

#[instrument(name = "command.recommend.read_cache_blocking", skip(cache_key), fields(cache_key = %cache_key))]
fn read_cached_recommendation_response(cache_key: String) -> RedisResult<String> {
    check_cache(&cache_key)
}

#[instrument(name = "command.recommend.write_cache_blocking", skip(cache_key, response), fields(cache_key = %cache_key))]
fn write_cached_recommendation_response(cache_key: String, response: String) {
    try_to_cache_response(&cache_key, &response)
}

#[instrument]
fn recommendation_cache_key(media_type: &MediaType, search_term: &str) -> String {
    format!("recommendation:{}:{search_term}", media_type.as_ref())
}

#[instrument(skip(media, recommendations))]
fn recommendations_embed(
    media: &RecommendationMedia,
    recommendations: &[(&Recommendation, &RecommendedMedia)],
    title_variant: Option<TitleVariant>,
) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .color(media.transform_color())
        .title(format!(
            "Recommendations for {}",
            media_title(media, title_variant)
        ))
        .url(media.transform_anilist())
        .footer(CreateEmbedFooter::new(format!(
            "Recommendations from AniList community ratings • {}",
            titlecase(media.get_type())
        )));

    let thumbnail = media.transform_thumbnail();
    if !thumbnail.is_empty() {
        embed = embed.thumbnail(thumbnail);
    }

    for (index, (recommendation, recommended_media)) in recommendations.iter().enumerate() {
        embed = embed.field(
            format!("{}. {}", index + 1, recommended_media.display_title()),
            format_recommendation(recommendation, recommended_media),
            false,
        );
    }

    embed
}

#[instrument(skip(recommendation, recommended_media))]
fn format_recommendation(
    recommendation: &Recommendation,
    recommended_media: &RecommendedMedia,
) -> String {
    let score = recommended_media
        .average_score()
        .map(|score| format!("{score}/100"))
        .unwrap_or_else(|| EMPTY_STR.to_string());
    let genres = recommended_media
        .genres()
        .iter()
        .take(3)
        .map(|genre| code(&titlecase(genre)))
        .collect::<Vec<_>>()
        .join(" - ");
    let genres = if genres.is_empty() {
        EMPTY_STR.to_string()
    } else {
        genres
    };

    format!(
        "{} • {} • {} • Score: {} • AniList rating: {}\nGenres: {}",
        linker("AniList", recommended_media.site_url()),
        titlecase(recommended_media.media_type()),
        format_media_descriptor(recommended_media),
        score,
        recommendation.rating_text(),
        genres,
    )
}

#[instrument(skip(recommended_media))]
fn format_media_descriptor(recommended_media: &RecommendedMedia) -> String {
    let format = recommended_media.format_text();
    let status = recommended_media.status_text();

    match (format, status) {
        (EMPTY_STR, EMPTY_STR) => EMPTY_STR.to_string(),
        (EMPTY_STR, status) => remove_underscores_and_titlecase(status),
        (format, EMPTY_STR) => remove_underscores_and_titlecase(format),
        (format, status) => format!(
            "{} / {}",
            remove_underscores_and_titlecase(format),
            remove_underscores_and_titlecase(status)
        ),
    }
}

#[instrument(skip(media))]
fn media_title(media: &RecommendationMedia, title_variant: Option<TitleVariant>) -> String {
    match title_variant {
        Some(TitleVariant::English) => media.transform_english_title(),
        Some(TitleVariant::Native) => media.transform_native_title(),
        Some(TitleVariant::Romaji) | None => media.transform_romaji_title(),
    }
}

#[instrument]
fn not_found_message(media_type: &MediaType) -> &'static str {
    match media_type {
        MediaType::Anime => NOT_FOUND_ANIME,
        MediaType::Manga => NOT_FOUND_MANGA,
    }
}

#[instrument]
fn anilist_type(media_type: &MediaType) -> &'static str {
    match media_type {
        MediaType::Anime => "ANIME",
        MediaType::Manga => "MANGA",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_media(is_adult: bool, recommendation_is_adult: bool) -> RecommendationMedia {
        serde_json::from_value(serde_json::json!({
            "type": "ANIME",
            "id": 1,
            "isAdult": is_adult,
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
                        "isAdult": recommendation_is_adult,
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
        }))
        .expect("sample recommendation media should deserialize")
    }

    fn sample_media_without_recommendations() -> RecommendationMedia {
        serde_json::from_value(serde_json::json!({
            "type": "MANGA",
            "id": 2,
            "isAdult": false,
            "title": {
                "romaji": "Yotsuba To!",
                "english": "Yotsuba&!",
                "native": "よつばと!"
            },
            "synonyms": [],
            "format": "MANGA",
            "status": "RELEASING",
            "genres": ["Comedy"],
            "coverImage": {
                "extraLarge": "https://example.com/base.jpg",
                "large": null,
                "medium": null,
                "color": "#abcdef"
            },
            "averageScore": 86,
            "siteUrl": "https://anilist.co/manga/2",
            "recommendations": { "nodes": [] }
        }))
        .expect("sample recommendation media should deserialize")
    }

    #[test]
    fn not_found_returns_type_specific_message() {
        let response = handle_recommend(None, MediaType::Manga, None, true);

        assert!(response.is_content());
        assert_eq!(response.unwrap_content(), NOT_FOUND_MANGA);
    }

    #[test]
    fn adult_base_media_is_blocked_when_adult_content_is_not_allowed() {
        let response = handle_recommend(
            Some(sample_media(true, false)),
            MediaType::Anime,
            None,
            false,
        );

        assert!(response.is_content());
        assert_eq!(response.unwrap_content(), NSFW_NOT_ALLOWED);
    }

    #[test]
    fn adult_recommendations_are_filtered_when_adult_content_is_not_allowed() {
        let response = handle_recommend(
            Some(sample_media(false, true)),
            MediaType::Anime,
            None,
            false,
        );

        assert!(response.is_content());
        assert!(
            response
                .unwrap_content()
                .contains("No recommendations found")
        );
    }

    #[test]
    fn no_recommendations_returns_content_message() {
        let response = handle_recommend(
            Some(sample_media_without_recommendations()),
            MediaType::Manga,
            Some(TitleVariant::English),
            true,
        );

        assert!(response.is_content());
        assert_eq!(
            response.unwrap_content(),
            "No recommendations found for Yotsuba&!."
        );
    }

    #[test]
    fn successful_recommendations_return_embed() {
        let response = handle_recommend(
            Some(sample_media(false, false)),
            MediaType::Anime,
            Some(TitleVariant::English),
            false,
        );

        assert!(response.is_embed());
        let embed = response.unwrap_embed();
        let value = serde_json::to_value(&embed).expect("embed serializes");
        assert_eq!(value["title"], "Recommendations for Cowboy Bebop");
        assert_eq!(value["fields"][0]["name"], "1. Samurai Champloo");
        assert!(
            value["fields"][0]["value"]
                .as_str()
                .unwrap()
                .contains("AniList rating: 42")
        );
    }
}
