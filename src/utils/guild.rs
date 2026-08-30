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
        requests::anilist::{AniListRequestError, send_request},
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

#[instrument(name = "guild.accept_partial_batch_response", skip(result))]
fn accept_partial_batch_response(
    result: Result<String, AniListRequestError>,
) -> Result<String, AniListRequestError> {
    match result {
        Err(AniListRequestError::NonSuccessStatus { status: 404, body })
            if serde_json::from_str::<BatchUserMediaListResponse>(&body)
                .is_ok_and(|response| response.data.is_some()) =>
        {
            Ok(body)
        }
        result => result,
    }
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
    get_guild_anilist_data_with_sender(guild_members, media_id, media_type, |body| async {
        accept_partial_batch_response(send_request(body).await)
    })
    .await
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
    use std::{cell::RefCell, collections::VecDeque, future::ready};

    use super::{
        ANILIST_MEDIA_LIST_BATCH_SIZE, accept_partial_batch_response, build_batch_media_list_query,
        get_guild_anilist_data_with_sender,
    };
    use crate::models::{
        db::oauth_credential::OAuthCredential,
        settings::{GuildScoresPreference, SettingValue, user_participates_in_guild_scores},
    };
    use crate::utils::requests::anilist::AniListRequestError;
    use serde_json::{Value, json};

    #[tracing::instrument(skip(discord_id, anilist_id))]
    fn credential(discord_id: impl ToString, anilist_id: i64) -> OAuthCredential {
        OAuthCredential {
            discord_user_id: discord_id.to_string(),
            anilist_id,
            anilist_username: None,
        }
    }

    #[tracing::instrument(skip(entries))]
    fn media_response(entries: &[(usize, u32)]) -> String {
        let data = entries
            .iter()
            .map(|(alias, score)| {
                (
                    format!("media_{alias}"),
                    json!({
                        "status": "COMPLETED",
                        "score": score,
                        "progress": 12,
                        "progressVolumes": null
                    }),
                )
            })
            .collect::<serde_json::Map<_, _>>();

        json!({ "data": data }).to_string()
    }

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

    #[tokio::test]
    async fn batch_size_boundary_uses_one_request_at_limit_and_two_above_it() {
        for (member_count, expected_request_count) in [
            (ANILIST_MEDIA_LIST_BATCH_SIZE, 1),
            (ANILIST_MEDIA_LIST_BATCH_SIZE + 1, 2),
        ] {
            let requests = RefCell::new(Vec::<Value>::new());
            let members = (1..=member_count)
                .map(|id| credential(id, id as i64 + 100))
                .collect();

            let result =
                get_guild_anilist_data_with_sender(members, 42, "anime".to_string(), |body| {
                    requests.borrow_mut().push(body);
                    ready(Ok::<_, &str>(json!({ "data": {} }).to_string()))
                })
                .await;

            assert!(result.is_empty());
            assert_eq!(requests.borrow().len(), expected_request_count);
            assert!(requests.borrow().iter().all(|body| {
                body["query"]
                    .as_str()
                    .is_some_and(|query| !query.contains("media_25:"))
            }));
        }
    }

    #[tokio::test]
    async fn multiple_batches_merge_results_with_batch_local_aliases() {
        let members = (1..=(ANILIST_MEDIA_LIST_BATCH_SIZE + 2))
            .map(|id| credential(id, id as i64 + 100))
            .collect();
        let responses = RefCell::new(VecDeque::from([
            Ok::<_, &str>(media_response(&[(0, 81), (24, 82)])),
            Ok(media_response(&[(0, 91), (1, 92)])),
        ]));

        let result = get_guild_anilist_data_with_sender(members, 42, "anime".to_string(), |_| {
            ready(
                responses
                    .borrow_mut()
                    .pop_front()
                    .expect("response per batch"),
            )
        })
        .await;

        assert_eq!(result.len(), 4);
        assert_eq!(result[&1].score, Some(81));
        assert_eq!(result[&25].score, Some(82));
        assert_eq!(result[&26].score, Some(91));
        assert_eq!(result[&27].score, Some(92));
    }

    #[tokio::test]
    async fn failed_batch_does_not_discard_successful_batch_results() {
        let members = (1..=(ANILIST_MEDIA_LIST_BATCH_SIZE + 1))
            .map(|id| credential(id, id as i64 + 100))
            .collect();
        let responses = RefCell::new(VecDeque::from([
            Ok::<_, &str>(media_response(&[(0, 75)])),
            Err("temporary failure"),
        ]));

        let result = get_guild_anilist_data_with_sender(members, 42, "anime".to_string(), |_| {
            ready(
                responses
                    .borrow_mut()
                    .pop_front()
                    .expect("response per batch"),
            )
        })
        .await;

        assert_eq!(result.len(), 1);
        assert_eq!(result[&1].score, Some(75));
    }

    #[tokio::test]
    async fn partial_not_found_response_keeps_available_member_data() {
        let members = vec![credential("1", 101), credential("2", 102)];
        let body = json!({
            "errors": [{ "message": "Not Found.", "status": 404 }],
            "data": {
                "media_0": {
                    "status": "COMPLETED",
                    "score": 88,
                    "progress": 12,
                    "progressVolumes": null
                },
                "media_1": null
            }
        })
        .to_string();

        let result = get_guild_anilist_data_with_sender(members, 42, "anime".to_string(), |_| {
            ready(accept_partial_batch_response(Err(
                AniListRequestError::NonSuccessStatus {
                    status: 404,
                    body: body.clone(),
                },
            )))
        })
        .await;

        assert_eq!(result.len(), 1);
        assert_eq!(result[&1].score, Some(88));
    }

    #[test]
    fn non_partial_not_found_response_remains_an_error() {
        let result = accept_partial_batch_response(Err(AniListRequestError::NonSuccessStatus {
            status: 404,
            body: r#"{"errors":[{"message":"Not Found.","status":404}]}"#.to_string(),
        }));

        assert!(matches!(
            result,
            Err(AniListRequestError::NonSuccessStatus { status: 404, .. })
        ));
    }

    #[tokio::test]
    async fn empty_and_invalid_credentials_do_not_send_requests() {
        for members in [
            Vec::new(),
            vec![
                credential("not-a-discord-id", 100),
                credential("1", 0),
                credential("2", i64::from(i32::MAX) + 1),
            ],
        ] {
            let request_count = RefCell::new(0);

            let result =
                get_guild_anilist_data_with_sender(members, 42, "anime".to_string(), |_| {
                    *request_count.borrow_mut() += 1;
                    ready(Ok::<_, &str>(json!({ "data": {} }).to_string()))
                })
                .await;

            assert!(result.is_empty());
            assert_eq!(*request_count.borrow(), 0);
        }
    }
}
