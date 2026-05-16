use serenity::all::{Channel, ChannelId, GuildId};
use serenity::client::Context;
use tracing::instrument;

/// Returns `true` when the given channel is marked as NSFW (age-restricted).
///
/// For guild text channels this checks the `nsfw` flag.
/// DM channels and any channels that cannot be resolved are treated as
/// **not** NSFW (i.e. adult content will be blocked).
#[instrument(name = "utils.channel.is_nsfw", skip(ctx), fields(channel_id = %channel_id))]
pub async fn is_nsfw_channel(
    ctx: &Context,
    channel_id: ChannelId,
    guild_id: Option<GuildId>,
) -> bool {
    if let Some(channel_nsfw) = guild_id
        .and_then(|guild_id| ctx.cache.guild(guild_id))
        .and_then(|guild| guild.channels.get(&channel_id).map(|channel| channel.nsfw))
    {
        return channel_nsfw;
    }

    match channel_id.to_channel(ctx).await {
        Ok(Channel::Guild(gc)) => gc.nsfw,
        _ => false,
    }
}
