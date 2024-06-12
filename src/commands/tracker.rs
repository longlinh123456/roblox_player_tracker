use super::{get_channel, Context};
use crate::{
    commands::{CommandError, CommandResult},
    constants::{GAME_LIMIT, TARGET_LIMIT},
    database::db,
    message_utils::{info_embed, success_message},
};
use poise::{
    command,
    serenity_prelude::{Mention, Role},
    CreateReply,
};

#[allow(clippy::unused_async)]
#[command(
    slash_command,
    subcommands("init", "info", "delete", "notify"),
    required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES",
    default_member_permissions = "MANAGE_CHANNELS",
    guild_only,
    ephemeral
)]
/// Operations on this channel's tracker
pub async fn tracker(_: Context<'_>) -> CommandResult {
    Ok(())
}

#[command(
    slash_command,
    required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES",
    default_member_permissions = "MANAGE_CHANNELS",
    guild_only,
    ephemeral
)]
/// Initialize the tracker in this channel
pub async fn init(ctx: Context<'_>) -> CommandResult {
    db().await
        .initialize(&ctx.guild_channel().await.unwrap())
        .await?;
    ctx.send(success_message(
        "Successfully initialized tracker in this channel",
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
/// View tracker info
pub async fn info(ctx: Context<'_>) -> CommandResult {
    let channel = get_channel(ctx.channel_id()).await?;
    let res = info_embed(format!(
        "Game count: {}/{GAME_LIMIT}
        Target count: {}/{TARGET_LIMIT}
        Notified role: {}",
        channel.game_count().await?,
        channel.target_count().await?,
        channel.notified_role().map_or_else(
            || String::from("none"),
            |role| Mention::Role(role).to_string()
        ),
    ))
    .title(format!(
        "Info for channel {}:",
        Mention::Channel(channel.id())
    ));
    ctx.send(CreateReply::default().embed(res)).await?;
    Ok(())
}
#[command(
    slash_command,
    required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES",
    default_member_permissions = "MANAGE_CHANNELS",
    guild_only,
    ephemeral
)]
/// Delete tracker
pub async fn delete(ctx: Context<'_>) -> CommandResult {
    let channel = get_channel(ctx.channel_id()).await?;
    let message_id = channel.message();
    channel.delete_channel().await?;
    if let Some(message_id) = message_id {
        if ctx
            .channel_id()
            .delete_message(ctx, message_id)
            .await
            .is_err()
        {
            return Err(CommandError::Expected(String::from(
                "Failed to delete the tracking output message.",
            )));
        }
    }
    ctx.send(success_message(
        "Succesfully deleted the tracker in this channel.",
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
/// Change notified role
pub async fn notify(
    ctx: Context<'_>,
    #[description = "The role to notify when targets are detected"] role: Option<Role>,
) -> CommandResult {
    let channel = get_channel(ctx.channel_id()).await?;
    channel
        .set_notified_role(role.as_ref().map(|role| role.id))
        .await?;
    if let Some(role) = role {
        ctx.send(success_message(format!(
            "Succesfully changed the notified role in this channel to {}.",
            Mention::Role(role.id)
        )))
        .await?;
    } else {
        ctx.send(success_message(
            "Succesfully cleared the notified role in this channel.",
        ))
        .await?;
    }
    Ok(())
}
