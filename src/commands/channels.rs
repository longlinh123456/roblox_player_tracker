use super::{CommandResult, Context};
use poise::{command, serenity_prelude::Mention};

use crate::{constants::CHANNEL_LIMIT, database::db, message_utils::render_lines_reply};

#[command(
    slash_command,
    required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES",
    default_member_permissions = "MANAGE_CHANNELS",
    guild_only,
    ephemeral
)]
/// Get all tracker channels in this server
pub async fn channels(ctx: Context<'_>) -> CommandResult {
    let res = db()
        .await
        .get_guild_channels(ctx.guild_id().unwrap())
        .await?;
    ctx.send(render_lines_reply(
        res.iter()
            .map(|channel| Mention::Channel(*channel.key()).to_string()),
        format!(
            "Tracker channels in this server ({}/{CHANNEL_LIMIT}):",
            res.len()
        ),
    ))
    .await?;
    Ok(())
}
