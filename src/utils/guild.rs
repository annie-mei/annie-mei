use std::collections::HashMap;

use crate::{
    models::{db::user::User, transformers::Transformers, user_media_list::MediaListData},
    utils::{database::establish_connection, score::get_guild_data},
};

use serenity::{
    client::Context,
    model::application::interaction::application_command::ApplicationCommandInteraction,
    model::prelude::{Guild, UserId},
};

use tracing::info;

fn get_guild_member_ids(guild: Guild) -> Vec<UserId> {
    let members: Vec<UserId> = guild.members.keys().copied().collect();
    info!("Found {:#?} members in guild", members.len());
    members
}

fn get_guild_from_interaction(ctx: &Context, interaction: &ApplicationCommandInteraction) -> Guild {
    let guild_id = interaction.guild_id.unwrap();
    guild_id.to_guild_cached(&ctx.cache).unwrap()
}

pub fn get_current_guild_members(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
) -> Vec<UserId> {
    let guild = get_guild_from_interaction(ctx, interaction);
    get_guild_member_ids(guild)
}

pub async fn get_guild_data_for_media<T: Transformers>(
    media: T,
    guild_members: Vec<UserId>,
) -> HashMap<i64, MediaListData> {
    let mut conn = establish_connection();
    let anilist_users = User::get_users_by_discord_id(guild_members, &mut conn);
    let anilist_users = anilist_users.unwrap();
    let guild_scores = get_guild_data(anilist_users, media.get_id(), media.get_type()).await;
    guild_scores
}
