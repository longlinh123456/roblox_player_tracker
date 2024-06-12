use super::{get_channel, Context};
use crate::{
    commands::{parse_id_list, CommandResult},
    constants::TARGET_LIMIT,
    message_utils::{render_lines_reply, success_message},
    roblox,
};
use poise::{
    command,
    serenity_prelude::{
        futures::{stream::FuturesUnordered, StreamExt},
        Mention,
    },
};

#[allow(clippy::unused_async)]
#[command(
    slash_command,
    subcommands("add", "remove", "view", "clear"),
    required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES",
    default_member_permissions = "MANAGE_CHANNELS",
    guild_only,
    ephemeral
)]
/// Operations on target list in this channel's tracker
pub async fn target(_: Context<'_>) -> CommandResult {
    Ok(())
}

#[command(
    slash_command,
    required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES",
    default_member_permissions = "MANAGE_CHANNELS",
    guild_only,
    ephemeral
)]
/// View targets
pub async fn view(ctx: Context<'_>) -> CommandResult {
    let channel = get_channel(ctx.channel_id()).await?;
    let lines = channel
        .get_targets()
        .await?
        .iter()
        .map(|id| async move { (*id, roblox::get_username(*id).await) })
        .collect::<FuturesUnordered<_>>()
        .map(|(id, line)| format!("[{line}](http://roblox.com/users/{id})"))
        .collect::<Vec<String>>()
        .await;
    ctx.send(render_lines_reply(
        lines,
        format!(
            "Targets for channel {} ({}/{TARGET_LIMIT}):",
            Mention::Channel(ctx.channel_id()),
            channel.target_count().await?
        ),
    ))
    .await?;
    Ok(())
}

#[command(
    slash_command,
    required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES",
    default_member_permissions = "MANAGE_CHANNELS",
    guild_only,
    ephemeral
)]
/// Add targets
pub async fn add(
    ctx: Context<'_>,
    #[description = "List of targets to add (comma seperated ids)"]
    #[min = 1]
    #[max = 1500]
    targets: String,
) -> CommandResult {
    let res = get_channel(ctx.channel_id())
        .await?
        .add_targets(parse_id_list(&targets))
        .await?;
    ctx.send(success_message(format!(
        "Inserted {res} targets into this channel's target list."
    )))
    .await?;
    Ok(())
}
#[command(
    slash_command,
    required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES",
    default_member_permissions = "MANAGE_CHANNELS",
    guild_only,
    ephemeral
)]
/// Remove targets
pub async fn remove(
    ctx: Context<'_>,
    #[description = "List of targets to remove (comma seperated ids)"]
    #[min = 1]
    targets: String,
) -> CommandResult {
    let res = get_channel(ctx.channel_id())
        .await?
        .remove_targets(parse_id_list(&targets))
        .await?;
    ctx.send(success_message(format!(
        "Removed {res} targets from this channel's target list."
    )))
    .await?;
    Ok(())
}
#[command(
    slash_command,
    required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES",
    default_member_permissions = "MANAGE_CHANNELS",
    guild_only,
    ephemeral
)]
/// Remove all targets
pub async fn clear(ctx: Context<'_>) -> CommandResult {
    let res = get_channel(ctx.channel_id()).await?.clear_targets().await?;
    ctx.send(success_message(format!(
        "Removed {res} targets from this channel's target list."
    )))
    .await?;
    Ok(())
}
