use crate::database::{db, CachedChannel, ChannelGetError};
use poise::serenity_prelude::{self, ChannelId};
use roblox_api::apis::Id;
use thiserror::Error;

pub mod channels;
pub mod games;
pub mod help;
pub mod register;
pub mod stats;
pub mod target;
pub mod tracker;

type Context<'a> = poise::Context<'a, (), CommandError>;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("{0}")]
    Expected(String),
    #[error(transparent)]
    Unexpected(anyhow::Error),
}

impl From<serenity_prelude::Error> for CommandError {
    fn from(value: serenity_prelude::Error) -> Self {
        Self::Unexpected(value.into())
    }
}

type CommandResult = Result<(), CommandError>;

async fn get_channel(channel: ChannelId) -> Result<CachedChannel, CommandError> {
    db().await
        .get_channel(channel)
        .await
        .map_err(|err| match err.as_ref() {
            ChannelGetError::Database(_) => CommandError::Unexpected(err.into()),
            ChannelGetError::NotInitialized => CommandError::Expected(err.to_string()),
        })
}

fn parse_list(list: &str) -> impl Iterator<Item = &str> + Clone {
    list.split(',').filter(|x| !x.is_empty()).map(str::trim)
}

fn parse_id_list(list: &str) -> impl Iterator<Item = Id> + Clone + '_ {
    parse_list(list).filter_map(|id| id.parse().ok())
}
