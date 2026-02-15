use std::collections::HashMap;

use crate::{
    models::{
        db::user::User,
        transformers::Transformers,
        user_media_list::{MediaListData, UserMediaList},
    },
    utils::{
        database::establish_connection, queries::FETCH_USER_MEDIA_LIST_DATA,
        requests::anilist::send_request,
    },
};

use serenity::{
    all::CommandInteraction,
    client::Context,
    model::prelude::{Guild, UserId},
};

use serde_json::json;
use tokio::task;
use tracing::{info, instrument};

#[instrument(name = "discord.guild.member_ids", skip(guild))]
fn get_guild_member_ids(guild: &Guild) -> Vec<UserId> {
    let members: Vec<UserId> = guild.members.keys().copied().collect();
    info!("Found {:#?} members in guild", members.len());
    members
}

#[instrument(name = "discord.guild.from_interaction", skip(ctx, interaction))]
fn get_guild_from_interaction(ctx: &Context, interaction: &CommandInteraction) -> Option<Guild> {
    interaction
        .guild_id
        .and_then(|guild_id| guild_id.to_guild_cached(&ctx.cache))
        // skipcq: RS-W1206 - CacheRef requires explicit clone via Deref
        .map(|g| g.clone())
}

#[instrument(name = "discord.guild.current_members", skip(ctx, interaction))]
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

#[instrument(name = "guild.fetch_anilist_data", skip(guild_members, media_type), fields(member_count = guild_members.len(), media_id, media_type = %media_type))]
async fn get_guild_anilist_data(
    guild_members: Vec<User>,
    media_id: u32,
    media_type: String,
) -> HashMap<i64, MediaListData> {
    let mut guild_members_data: HashMap<i64, MediaListData> = HashMap::new();
    for user in guild_members {
        let body = json!({
            "query": FETCH_USER_MEDIA_LIST_DATA,
            "variables": {
                "userId": user.anilist_id,
                "type": media_type.to_uppercase(),
                "mediaId": media_id
            }
        });
        info!("Body: {:#?}", body);
        let user_media_list_response = task::spawn_blocking(move || send_request(body))
            .await
            .unwrap();
        let user_media_list_response: UserMediaList =
            serde_json::from_str(&user_media_list_response).unwrap();

        let media_list_data = user_media_list_response.data.unwrap();
        match media_list_data.media_list {
            None => continue,
            Some(data) => {
                guild_members_data.insert(user.discord_id, data);
            }
        };
    }
    guild_members_data
}
