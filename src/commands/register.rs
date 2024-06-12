use super::{CommandResult, Context};
use poise::{builtins, command};

#[command(prefix_command, owners_only)]
pub async fn register(ctx: Context<'_>) -> CommandResult {
    builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}
