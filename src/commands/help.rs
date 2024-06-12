use super::{CommandResult, Context};
use poise::{builtins, command, samples::HelpConfiguration};

/// An overview of the tracker's commands
#[command(slash_command, ephemeral)]
pub async fn help(ctx: Context<'_>) -> CommandResult {
    builtins::help(ctx, None, HelpConfiguration {show_subcommands: true, extra_text_at_bottom: "Use this extension to use the follow links: https://chromewebstore.google.com/detail/roblox-url-launcher/lcefjaknjehbafdeacjbjnfpfldjdlcc", ..Default::default()}).await?;
    Ok(())
}
