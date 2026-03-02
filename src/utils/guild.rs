use std::collections::HashMap;

use crate::{
    models::{db::user::User, transformers::Transformers, user_media_list::MediaListData},
    utils::{database::establish_connection, requests::anilist::send_request},
};

use serenity::{
    all::CommandInteraction,
    client::Context,
    model::prelude::{Guild, UserId},
};

use serde::Deserialize;
use serde_json::json;
use tokio::task;
use tracing::{info, instrument};

#[derive(Deserialize, Debug)]
struct BatchUserMediaListResponse {
    data: Option<HashMap<String, Option<MediaListData>>>,
}

const MEDIA_LIST_QUERY_FIELDS: &str = "status\nscore(format: POINT_100)\nprogress\nprogressVolumes";

#[instrument(name = "guild.media_alias")]
fn media_alias(index: usize) -> String {
    format!("media_{index}")
}

#[instrument(name = "guild.build_batch_media_list_query", skip(guild_members), fields(member_count = guild_members.len()))]
fn build_batch_media_list_query(guild_members: &[User]) -> String {
    let media_lookups = guild_members
        .iter()
        .enumerate()
        .map(|(index, user)| {
            format!(
                "  {}: MediaList(userId: {}, type: $type, mediaId: $mediaId) {{\n    {}\n  }}",
                media_alias(index),
                user.anilist_id,
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

#[instrument(name = "guild.fetch_media_data", skip(media, guild_members), fields(member_count = guild_members.len()))]
pub async fn get_guild_data_for_media<T: Transformers>(
    media: T,
    guild_members: Vec<UserId>,
) -> HashMap<i64, MediaListData> {
    let mut conn = establish_connection();
    let anilist_users = User::get_users_by_discord_id(guild_members, &mut conn);
    let anilist_users = anilist_users.unwrap();

    get_guild_anilist_data(anilist_users, media.get_id(), media.get_type()).await
}

#[instrument(name = "guild.fetch_anilist_data", skip(guild_members, media_type), fields(member_count = guild_members.len(), media_id = media_id, media_type = %media_type))]
async fn get_guild_anilist_data(
    guild_members: Vec<User>,
    media_id: u32,
    media_type: String,
) -> HashMap<i64, MediaListData> {
    if guild_members.is_empty() {
        return HashMap::new();
    }

    let discord_ids_by_media_alias: HashMap<String, i64> = guild_members
        .iter()
        .enumerate()
        .map(|(index, user)| (media_alias(index), user.discord_id))
        .collect();

    let query = build_batch_media_list_query(&guild_members);

    let body = json!({
        "query": query,
        "variables": {
            "type": media_type.to_uppercase(),
            "mediaId": media_id
        }
    });

    info!("Body: {:#?}", body);
    let user_media_list_response = task::spawn_blocking(move || send_request(body))
        .await
        .unwrap();

    let user_media_list_response: BatchUserMediaListResponse =
        serde_json::from_str(&user_media_list_response).unwrap();

    let mut guild_members_data: HashMap<i64, MediaListData> = HashMap::new();
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

    guild_members_data
}

#[cfg(test)]
mod tests {
    use super::build_batch_media_list_query;
    use crate::models::db::user::User;

    #[test]
    fn build_batch_media_list_query_adds_one_lookup_per_user() {
        let guild_members = vec![
            User {
                discord_id: 1,
                anilist_id: 100,
                anilist_username: "first".to_string(),
            },
            User {
                discord_id: 2,
                anilist_id: 200,
                anilist_username: "second".to_string(),
            },
        ];

        let query = build_batch_media_list_query(&guild_members);

        assert!(query.contains("media_0: MediaList(userId: 100, type: $type, mediaId: $mediaId)"));
        assert!(query.contains("media_1: MediaList(userId: 200, type: $type, mediaId: $mediaId)"));
    }
}
