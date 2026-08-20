use std::{collections::HashMap, fmt::Display, future::Future};

use crate::{
    models::{
        db::{oauth_credential::OAuthCredential, settings::get_user_settings_for_discord_ids},
        settings::SettingKey,
        transformers::Transformers,
        user_media_list::MediaListData,
    },
    utils::{
        database::get_pool_from_context,
        requests::anilist::send_request,
        settings::{participates_in_guild_scores, resolve_guild_scores_enabled_with_pool},
    },
};

use serenity::{
    all::CommandInteraction,
    client::Context,
    model::prelude::{Guild, GuildId, UserId},
};

use serde::Deserialize;
use serde_json::json;
use tracing::{error, info, instrument};

#[derive(Deserialize, Debug)]
struct BatchUserMediaListResponse {
    data: Option<HashMap<String, Option<MediaListData>>>,
}

const MEDIA_LIST_QUERY_FIELDS: &str = "status\nscore(format: POINT_100)\nprogress\nprogressVolumes";
// Each lookup is a root resolver returning four scalar fields. Twenty-five
// lookups therefore cap a request at 100 fields and roughly 4 KiB of query
// text, while avoiding a separate rate-limited AniList request per member.
const ANILIST_MEDIA_LIST_BATCH_SIZE: usize = 25;

#[instrument(name = "guild.media_alias")]
fn media_alias(index: usize) -> String {
    format!("media_{index}")
}

#[instrument(name = "guild.build_batch_media_list_query", skip(guild_members), fields(member_count = guild_members.len()))]
fn build_batch_media_list_query(guild_members: &[OAuthCredential]) -> String {
    let media_lookups = guild_members
        .iter()
        .enumerate()
        .map(|(index, credential)| {
            format!(
                "  {}: MediaList(userId: {}, type: $type, mediaId: $mediaId) {{\n    {}\n  }}",
                media_alias(index),
                credential.anilist_id,
                MEDIA_LIST_QUERY_FIELDS
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "query ($type: MediaType, $mediaId: Int) {{\n{}\n}}",
        media_lookups
    )
}

#[instrument(name = "discord.guild.member_ids", skip(guild), fields(member_count = guild.members.len()))]
fn get_guild_member_ids(guild: &Guild) -> Vec<UserId> {
    let members: Vec<UserId> = guild.members.keys().copied().collect();
    info!("Found {:#?} members in guild", members.len());
    members
}

#[instrument(name = "discord.guild.from_interaction", skip(ctx, interaction), fields(has_guild_id = interaction.guild_id.is_some()))]
fn get_guild_from_interaction(ctx: &Context, interaction: &CommandInteraction) -> Option<Guild> {
    interaction
        .guild_id
        .and_then(|guild_id| guild_id.to_guild_cached(&ctx.cache))
        // skipcq: RS-W1206 - CacheRef requires explicit clone via Deref
        .map(|g| g.clone())
}

#[instrument(name = "discord.guild.current_members", skip(ctx, interaction), fields(has_guild_id = interaction.guild_id.is_some()))]
pub fn get_current_guild_members(ctx: &Context, interaction: &CommandInteraction) -> Vec<UserId> {
    get_guild_from_interaction(ctx, interaction)
        .as_ref()
        .map(get_guild_member_ids)
        .unwrap_or_default()
}

#[instrument(name = "guild.fetch_media_data", skip(ctx, media, guild_members), fields(member_count = guild_members.len()))]
pub async fn get_guild_data_for_media<T: Transformers>(
    ctx: &Context,
    media: &T,
    guild_id: Option<GuildId>,
    guild_members: Vec<UserId>,
) -> HashMap<u64, MediaListData> {
    let Some(database_pool) = get_pool_from_context(ctx).await else {
        error!("Database pool is not available in Serenity context");
        return HashMap::new();
    };

    if !resolve_guild_scores_enabled_with_pool(&database_pool, guild_id).await {
        info!("Guild scores are disabled for this interaction");
        return HashMap::new();
    }

    let anilist_users =
        match OAuthCredential::get_by_discord_ids(guild_members, &database_pool).await {
            Ok(users) => users,
            Err(err) => {
                error!(error = %err, "Failed to fetch registered guild members from database");
                return HashMap::new();
            }
        };

    let participating_users =
        match filter_guild_score_participants(anilist_users, &database_pool).await {
            Ok(users) => users,
            Err(err) => {
                error!(error = %err, "Failed to resolve guild score opt-outs");
                return HashMap::new();
            }
        };

    get_guild_anilist_data(
        participating_users,
        media.get_id(),
        media.get_type().to_owned(),
    )
    .await
}

#[instrument(name = "guild.filter_score_participants", skip(guild_members, database_pool), fields(member_count = guild_members.len()))]
async fn filter_guild_score_participants(
    guild_members: Vec<OAuthCredential>,
    database_pool: &crate::utils::database::DbPool,
) -> Result<Vec<OAuthCredential>, crate::models::db::settings::SettingsStorageError> {
    let discord_user_ids = guild_members
        .iter()
        .map(|credential| credential.discord_user_id.clone())
        .collect::<Vec<_>>();
    let user_settings = get_user_settings_for_discord_ids(
        database_pool,
        &discord_user_ids,
        SettingKey::GuildScores,
    )
    .await?;

    Ok(guild_members
        .into_iter()
        .filter(|credential| {
            participates_in_guild_scores(user_settings.get(&credential.discord_user_id).copied())
        })
        .collect())
}

#[instrument(name = "guild.fetch_anilist_data", skip(guild_members, media_type), fields(member_count = guild_members.len(), media_id = media_id, media_type = %media_type))]
async fn get_guild_anilist_data(
    guild_members: Vec<OAuthCredential>,
    media_id: u32,
    media_type: String,
) -> HashMap<u64, MediaListData> {
    get_guild_anilist_data_with_sender(guild_members, media_id, media_type, send_request).await
}

#[instrument(name = "guild.fetch_anilist_data_batches", skip(guild_members, media_type, sender), fields(member_count = guild_members.len(), media_id = media_id, media_type = %media_type))]
async fn get_guild_anilist_data_with_sender<F, Fut, E>(
    guild_members: Vec<OAuthCredential>,
    media_id: u32,
    media_type: String,
    mut sender: F,
) -> HashMap<u64, MediaListData>
where
    F: FnMut(serde_json::Value) -> Fut,
    Fut: Future<Output = Result<String, E>>,
    E: Display,
{
    // Invalid stored identifiers cannot be mapped safely and an out-of-range
    // AniList ID is not valid for GraphQL's signed 32-bit Int type.
    let valid_guild_members: Vec<_> = guild_members
        .into_iter()
        .filter(|credential| {
            credential.discord_id_u64().is_some()
                && i32::try_from(credential.anilist_id).is_ok_and(|id| id > 0)
        })
        .collect();

    let mut guild_members_data: HashMap<u64, MediaListData> = HashMap::new();
    let batch_count = valid_guild_members
        .len()
        .div_ceil(ANILIST_MEDIA_LIST_BATCH_SIZE);

    for (batch_index, guild_member_batch) in valid_guild_members
        .chunks(ANILIST_MEDIA_LIST_BATCH_SIZE)
        .enumerate()
    {
        // Aliases restart in each independent query, so build the reverse map
        // from this exact slice rather than from the full guild member list.
        let discord_ids_by_media_alias: HashMap<String, u64> = guild_member_batch
            .iter()
            .enumerate()
            .filter_map(|(index, credential)| {
                credential
                    .discord_id_u64()
                    .map(|discord_id| (media_alias(index), discord_id))
            })
            .collect();
        let query = build_batch_media_list_query(guild_member_batch);

        let body = json!({
            "query": query,
            "variables": {
                "type": media_type.to_uppercase(),
                "mediaId": media_id
            }
        });

        info!(
            batch_number = batch_index + 1,
            batch_count,
            batch_member_count = guild_member_batch.len(),
            request_body_len = body.to_string().len(),
            "Sending batch AniList media list query"
        );
        let user_media_list_response = match sender(body).await {
            Ok(response) => response,
            Err(err) => {
                error!(
                    error = %err,
                    batch_number = batch_index + 1,
                    batch_count,
                    "AniList batch media list request failed"
                );
                continue;
            }
        };

        let user_media_list_response: BatchUserMediaListResponse =
            match serde_json::from_str::<BatchUserMediaListResponse>(&user_media_list_response) {
                Ok(response) => response,
                Err(err) => {
                    error!(
                        error = %err,
                        batch_number = batch_index + 1,
                        batch_count,
                        "Failed to parse guild AniList media data response"
                    );
                    continue;
                }
            };

        if let Some(media_lookup_data) = user_media_list_response.data {
            for (media_alias, media_list_data) in media_lookup_data {
                if let (Some(discord_id), Some(data)) = (
                    discord_ids_by_media_alias.get(&media_alias),
                    media_list_data,
                ) {
                    guild_members_data.insert(*discord_id, data);
                }
            }
        }
    }

    guild_members_data
}

#[cfg(test)]
mod tests {
    use super::build_batch_media_list_query;
    use crate::models::{
        db::oauth_credential::OAuthCredential,
        settings::{GuildScoresPreference, SettingValue, user_participates_in_guild_scores},
    };

    #[test]
    fn guild_score_participation_defaults_to_include_and_honors_opt_out() {
        assert!(user_participates_in_guild_scores(None));
        assert!(user_participates_in_guild_scores(Some(
            SettingValue::GuildScores(GuildScoresPreference::Enabled)
        )));
        assert!(!user_participates_in_guild_scores(Some(
            SettingValue::GuildScores(GuildScoresPreference::OptedOut)
        )));
    }

    #[test]
    fn build_batch_media_list_query_adds_one_lookup_per_user() {
        let guild_members = vec![
            OAuthCredential {
                discord_user_id: "1".to_string(),
                anilist_id: 100,
                anilist_username: Some("UserOne".to_string()),
            },
            OAuthCredential {
                discord_user_id: "2".to_string(),
                anilist_id: 200,
                anilist_username: Some("UserTwo".to_string()),
            },
        ];

        let query = build_batch_media_list_query(&guild_members);

        assert!(query.contains("media_0: MediaList(userId: 100, type: $type, mediaId: $mediaId)"));
        assert!(query.contains("media_1: MediaList(userId: 200, type: $type, mediaId: $mediaId)"));
    }
}
