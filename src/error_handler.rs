use crate::{commands::CommandError, message_utils::failure_embed};
use anyhow::Result;
use poise::{CreateReply, FrameworkError};
use tracing::{error, warn};

#[allow(clippy::too_many_lines)]
pub async fn handle<T: Send + Sync>(error: FrameworkError<'_, T, CommandError>) -> Result<()> {
    match error {
        FrameworkError::Setup { error, .. } => {
            error!("Error in user data setup: {:?}", error);
        }
        FrameworkError::EventHandler { error, event, .. } => error!(
            "User event handler encountered an error on {} event: {}",
            event.snake_case_name(),
            error
        ),
        FrameworkError::Command { ctx, error, .. } => {
            let error_msg = match error {
                CommandError::Expected(msg) => msg,
                CommandError::Unexpected(err) => {
                    error!(
                        "An unexpected error occured in command {}: {:?}",
                        ctx.command().name,
                        &err
                    );
                    format!("This command encountered an unexpected error:\n {err}")
                }
            };
            ctx.send(
                CreateReply::default()
                    .embed(failure_embed(error_msg))
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::SubcommandRequired { ctx } => {
            let subcommands = ctx
                .command()
                .subcommands
                .iter()
                .map(|s| &*s.name)
                .collect::<Vec<_>>();
            let response = format!(
                "You must specify one of the following subcommands: {}",
                subcommands.join(", ")
            );
            ctx.send(
                CreateReply::default()
                    .embed(failure_embed(response))
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::CommandPanic { ctx, payload, .. } => {
            // Not showing the payload to the user because it may contain sensitive info
            error!(
                "Command {} panicked with payload: {:?}",
                ctx.command().name,
                payload
            );
            ctx.send(
                CreateReply::default()
                    .embed(failure_embed("An unexpected internal error has occurred."))
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::ArgumentParse {
            ctx, input, error, ..
        } => {
            // If we caught an argument parse error, give a helpful error message with the
            // command explanation if available
            let usage = ctx.command().help_text.as_ref().map_or(
                "Please check the help menu for usage information.",
                |help_text| &**help_text,
            );
            let response = input.map_or_else(
                || format!("**{error}**\n{usage}"),
                |input| format!("**Cannot parse `{input}` as argument: {error}**\n{usage}"),
            );
            ctx.send(
                CreateReply::default()
                    .embed(failure_embed(response))
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::CommandStructureMismatch {
            ctx, description, ..
        } => {
            error!(
                "Failed to deserialize interaction arguments for `/{}`: {}",
                ctx.command.name, description,
            );
        }
        FrameworkError::CommandCheckFailed { ctx, error, .. } => {
            error!(
                "Command check failed in command {} for user {}: {:?}",
                ctx.command().name,
                ctx.author().name,
                error,
            );
        }
        FrameworkError::CooldownHit {
            remaining_cooldown,
            ctx,
            ..
        } => {
            let msg = format!(
                "You're too fast. Please wait {} seconds before retrying.",
                remaining_cooldown.as_secs()
            );
            ctx.send(
                CreateReply::default()
                    .embed(failure_embed(msg))
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::MissingBotPermissions {
            missing_permissions,
            ctx,
            ..
        } => {
            let msg = format!(
                "Command cannot be executed because the bot is lacking permissions: {missing_permissions}",
            );
            ctx.send(
                CreateReply::default()
                    .embed(failure_embed(msg))
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::MissingUserPermissions {
            missing_permissions,
            ctx,
            ..
        } => {
            let response = missing_permissions.map_or_else(
                || {
                    format!(
                        "You may be lacking permissions for `{}{}`. This command cannot be executed for safety.",
                        ctx.prefix(),
                        ctx.command().name,
                    )
                },
                |missing_permissions| {
                    format!(
                        "You're lacking permissions for `{}{}`: {}",
                        ctx.prefix(),
                        ctx.command().name,
                        missing_permissions,
                    )
                },
            );
            ctx.send(
                CreateReply::default()
                    .embed(failure_embed(response))
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::NotAnOwner { ctx, .. } => {
            let response = "Only bot owners can call this command.";
            ctx.send(
                CreateReply::default()
                    .embed(failure_embed(response))
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::GuildOnly { ctx, .. } => {
            let response = "You cannot run this command in DMs.";
            ctx.send(
                CreateReply::default()
                    .embed(failure_embed(response))
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::DmOnly { ctx, .. } => {
            let response = "You cannot run this command outside DMs.";
            ctx.send(
                CreateReply::default()
                    .embed(failure_embed(response))
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::NsfwOnly { ctx, .. } => {
            let response = "You cannot run this command outside NSFW channels.";
            ctx.send(
                CreateReply::default()
                    .embed(failure_embed(response))
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::DynamicPrefix { error, msg, .. } => {
            error!(
                "Dynamic prefix failed for message {:?}: {}",
                msg.content, error
            );
        }
        FrameworkError::UnknownCommand {
            msg_content,
            prefix,
            ..
        } => {
            warn!(
                "Recognized prefix `{}`, but didn't recognize command name in `{}`",
                prefix, msg_content,
            );
        }
        FrameworkError::UnknownInteraction { interaction, .. } => {
            warn!("Received unknown interaction \"{}\"", interaction.data.name);
        }
        _ => {}
    }
    Ok(())
}
