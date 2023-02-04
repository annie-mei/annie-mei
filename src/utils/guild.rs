use serenity::model::prelude::{Guild, UserId};

pub fn get_guild_member_ids(guild: Guild) -> Vec<UserId> {
    guild.members.keys().map(|id| *id).collect()
}
