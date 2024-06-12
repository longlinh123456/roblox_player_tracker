#![deny(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::unreadable_literal,
    clippy::cast_possible_wrap
)]

mod commands;
mod constants;
mod database;
mod error_handler;
mod message_utils;
mod roblox;

use std::env;

use anyhow::{Context, Result};
use commands::{channels, games, help, stats, target, tracker};
use poise::{
    builtins,
    serenity_prelude::{ClientBuilder, Command, CreateAllowedMentions, GatewayIntents},
    Framework, FrameworkOptions,
};
use roblox::{tracking, update};
use tokio::task;
use tracing::error;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();
    let options = FrameworkOptions {
        commands: vec![
            channels::channels(),
            games::game(),
            target::target(),
            tracker::tracker(),
            help::help(),
            stats::stats(),
        ],
        on_error: |err| {
            Box::pin(async move {
                if let Err(err) = error_handler::handle(err).await {
                    error!("Error while handling error: {}", err);
                }
            })
        },
        allowed_mentions: Some(
            CreateAllowedMentions::new()
                .all_roles(true)
                .all_users(false)
                .replied_user(true),
        ),
        ..Default::default()
    };
    let framework = Framework::builder()
        .setup(|ctx, _, framework| {
            Box::pin(async move {
                task::spawn(tracking::tracking_loop());
                task::spawn({
                    let cache = ctx.cache.clone();
                    let http = ctx.http.clone();
                    update::update_loop(cache, http)
                });
                Command::set_global_commands(
                    ctx,
                    builtins::create_application_commands(&framework.options().commands),
                )
                .await?;
                Ok(())
            })
        })
        .options(options)
        .build();
    let mut client = ClientBuilder::new(
        env::var("TOKEN").context("failed to get bot token")?,
        GatewayIntents::non_privileged(),
    )
    .framework(framework)
    .await?;
    Ok(client.start().await?)
}
