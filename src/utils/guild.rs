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
    client::Context,
    model::application::interaction::application_command::ApplicationCommandInteraction,
    model::prelude::{Guild, UserId},
};

use serde_json::json;
use tokio::task;
use tracing::info;

fn get_guild_member_ids(guild: Guild) -> Vec<UserId> {
    let members: Vec<UserId> = guild.members.keys().copied().collect();
    info!("Found {:#?} members in guild", members.len());
    members
}

fn get_guild_from_interaction(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
) -> Option<Guild> {
    match interaction.guild_id {
        None => None,
        Some(guild_id) => guild_id.to_guild_cached(&ctx.cache),
    }
}

pub fn get_current_guild_members(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
) -> Vec<UserId> {
    let guild = get_guild_from_interaction(ctx, interaction);
    match guild {
        None => vec![],
        Some(guild) => get_guild_member_ids(guild),
    }
}

pub async fn get_guild_data_for_media<T: Transformers>(
    media: T,
    guild_members: Vec<UserId>,
) -> HashMap<i64, MediaListData> {
    let mut conn = establish_connection();
    let anilist_users = User::get_users_by_discord_id(guild_members, &mut conn);
    let anilist_users = anilist_users.unwrap();
    let guild_members_data =
        get_guild_anilist_data(anilist_users, media.get_id(), media.get_type()).await;
    guild_members_data
}

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
