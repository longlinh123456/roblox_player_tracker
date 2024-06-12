use super::{get_channel, Context};
use crate::{
    commands::{parse_id_list, CommandResult},
    constants::GAME_LIMIT,
    message_utils::{render_lines_reply, success_message},
    roblox,
};
use poise::{
    command,
    serenity_prelude::{
        futures::stream::{FuturesUnordered, StreamExt},
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
/// Operations on game list in this channel's tracker
pub async fn game(_: Context<'_>) -> CommandResult {
    Ok(())
}

#[command(
    slash_command,
    required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES",
    default_member_permissions = "MANAGE_CHANNELS",
    guild_only,
    ephemeral
)]
/// View games
pub async fn view(ctx: Context<'_>) -> CommandResult {
    let channel = get_channel(ctx.channel_id()).await?;
    let lines = channel
        .get_games()
        .await?
        .iter()
        .map(|id| async move { (*id, roblox::get_game_name(*id).await) })
        .collect::<FuturesUnordered<_>>()
        .map(|(id, line)| format!("[{line}](http://roblox.com/games/{id})"))
        .collect::<Vec<String>>()
        .await;
    ctx.send(render_lines_reply(
        lines,
        format!(
            "Games for channel {} ({}/{GAME_LIMIT}):",
            Mention::Channel(ctx.channel_id()),
            channel.game_count().await?
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
/// Add games
pub async fn add(
    ctx: Context<'_>,
    #[description = "List of games to add (comma seperated ids)"]
    #[min = 1]
    #[max = 1500]
    games: String,
) -> CommandResult {
    let res = get_channel(ctx.channel_id())
        .await?
        .add_games(parse_id_list(&games))
        .await?;
    ctx.send(success_message(format!(
        "Inserted {res} games into this channel's game list."
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
/// Remove games
pub async fn remove(
    ctx: Context<'_>,
    #[description = "List of games to remove (comma seperated ids)"]
    #[min = 1]
    games: String,
) -> CommandResult {
    let res = get_channel(ctx.channel_id())
        .await?
        .remove_games(parse_id_list(&games))
        .await?;
    ctx.send(success_message(format!(
        "Removed {res} games from this channel's game list."
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
/// Remove all games
pub async fn clear(ctx: Context<'_>) -> CommandResult {
    let res = get_channel(ctx.channel_id()).await?.clear_games().await?;
    ctx.send(success_message(format!(
        "Removed {res} games from this channel's game list."
    )))
    .await?;
    Ok(())
}
