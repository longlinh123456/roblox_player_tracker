#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use crate::{
    commands::CommandError,
    constants::{CHANNEL_LIMIT, DATABASE_URL, GAME_LIMIT, TARGET_LIMIT},
};
use ahash::{HashMap, RandomState};
use arc_swap::ArcSwapOption;
use dashmap::DashSet;
use delegate::delegate;
use entities::{channel, game, prelude::*, target};
use migration::{Migrator, MigratorTrait};
use moka::future::Cache;
use poise::serenity_prelude::{ChannelId, GuildChannel, GuildId, MessageId, RoleId};
use roblox_api::apis::Id;
use sea_orm::{
    prelude::*,
    ActiveValue::{NotSet, Set},
    JoinType, QuerySelect,
};
use sea_query::OnConflict;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::OnceCell;

static DATABASE: OnceCell<Database> = OnceCell::const_new();

pub async fn db() -> &'static Database {
    DATABASE
        .get_or_init(|| async {
            Database::new()
                .await
                .expect("Database should initialize successfully")
        })
        .await
}

#[derive(Debug, Error)]
pub enum GameInsertError {
    #[error("database error: {0}")]
    Database(DbErr),
    #[error("Game limit exceeded (games after adding: {0}/{}).", GAME_LIMIT)]
    LimitExceeded(usize),
    #[error("Provided game list is empty.")]
    GameListEmpty,
    #[error("All the provided games were already in the tracker list.")]
    GamesNotInserted,
}

impl From<DbErr> for GameInsertError {
    fn from(value: DbErr) -> Self {
        if value == DbErr::RecordNotInserted {
            Self::GamesNotInserted
        } else {
            Self::Database(value)
        }
    }
}

impl From<GameInsertError> for CommandError {
    fn from(value: GameInsertError) -> Self {
        match value {
            GameInsertError::Database(err) => Self::Unexpected(err.into()),
            _ => Self::Expected(value.to_string()),
        }
    }
}

#[derive(Debug, Error)]
pub enum TargetInsertError {
    #[error("database error: {0}")]
    Database(DbErr),
    #[error("Target limit exceeded (targets after adding: {0}/{}).", TARGET_LIMIT)]
    LimitExceeded(usize),
    #[error("Provided target list is empty.")]
    TargetListEmpty,
    #[error("All the provided targets were already in the tracker list.")]
    TargetsNotInserted,
}

impl From<DbErr> for TargetInsertError {
    fn from(value: DbErr) -> Self {
        if value == DbErr::RecordNotInserted {
            Self::TargetsNotInserted
        } else {
            Self::Database(value)
        }
    }
}

impl From<TargetInsertError> for CommandError {
    fn from(value: TargetInsertError) -> Self {
        match value {
            TargetInsertError::Database(err) => Self::Unexpected(err.into()),
            _ => Self::Expected(value.to_string()),
        }
    }
}

#[derive(Debug, Error)]
pub enum ChannelDeleteError {
    #[error("database error: {0}")]
    Database(#[from] DbErr),
    #[error("There are still operations running on this channel's tracker.")]
    OperationPending,
}

impl From<ChannelDeleteError> for CommandError {
    fn from(value: ChannelDeleteError) -> Self {
        match value {
            ChannelDeleteError::Database(err) => Self::Unexpected(err.into()),
            _ => Self::Expected(value.to_string()),
        }
    }
}

#[derive(Debug, Error)]
pub enum GameDeleteError {
    #[error("database error: {0}")]
    Database(#[from] DbErr),
    #[error("No games were deleted by this command.")]
    GamesNotDeleted,
}

impl From<GameDeleteError> for CommandError {
    fn from(value: GameDeleteError) -> Self {
        match value {
            GameDeleteError::Database(err) => Self::Unexpected(err.into()),
            _ => Self::Expected(value.to_string()),
        }
    }
}

#[derive(Debug, Error)]
pub enum TargetDeleteError {
    #[error("database error: {0}")]
    Database(#[from] DbErr),
    #[error("No targets were deleted by this command.")]
    TargetsNotDeleted,
}

impl From<TargetDeleteError> for CommandError {
    #[allow(clippy::match_wildcard_for_single_variants)]
    fn from(value: TargetDeleteError) -> Self {
        match value {
            TargetDeleteError::Database(err) => Self::Unexpected(err.into()),
            _ => Self::Expected(value.to_string()),
        }
    }
}

impl From<DbErr> for CommandError {
    fn from(value: DbErr) -> Self {
        Self::Unexpected(value.into())
    }
}

impl From<Arc<DbErr>> for CommandError {
    fn from(value: Arc<DbErr>) -> Self {
        Self::Unexpected(value.into())
    }
}

#[derive(Clone)]
pub struct CachedChannel {
    inner: Arc<InnerCachedChannel>,
}

impl CachedChannel {
    fn new(channel: &QueriedChannel) -> Self {
        Self {
            inner: Arc::new(InnerCachedChannel::new(channel)),
        }
    }
    pub async fn delete_channel(self) -> Result<(), ChannelDeleteError> {
        db().await.delete_channel(self.inner).await
    }
    delegate! {
        to self.inner {
            pub fn id(&self) -> ChannelId;
            pub fn message(&self) -> Option<MessageId>;
            pub fn notified_role(&self) -> Option<RoleId>;
            pub fn guild(&self) -> GuildId;
            pub async fn get_targets(&self) -> Result<&DashSet<Id, RandomState>, DbErr>;
            pub async fn get_games(&self) -> Result<&DashSet<Id, RandomState>, DbErr>;
            pub async fn add_targets(
                &self,
                targets: impl IntoIterator<Item = Id> + Send,
            ) -> Result<usize, TargetInsertError>;
            pub async fn add_games(
                &self,
                games: impl IntoIterator<Item = Id> + Send,
            ) -> Result<usize, GameInsertError>;
            pub async fn remove_targets(
                &self,
                targets: impl IntoIterator<Item = Id> + Send + Clone,
            ) -> Result<usize, TargetDeleteError>;
            pub async fn remove_games(
                &self,
                games: impl IntoIterator<Item = Id> + Send + Clone,
            ) -> Result<usize, GameDeleteError>;
            pub async fn clear_targets(&self) -> Result<usize, TargetDeleteError>;
            pub async fn clear_games(&self) -> Result<usize, GameDeleteError>;
            pub async fn game_count(&self) -> Result<usize, DbErr>;
            pub async fn target_count(&self) -> Result<usize, DbErr>;
            pub async fn set_message(&self, message: MessageId) -> Result<(), DbErr>;
            pub async fn set_notified_role(&self, role: Option<RoleId>) -> Result<(), DbErr>;
        }
    }
}

struct InnerCachedChannel {
    channel: ChannelId,
    targets: OnceCell<DashSet<Id, RandomState>>,
    games: OnceCell<DashSet<Id, RandomState>>,
    guild: GuildId,
    message: ArcSwapOption<MessageId>,
    notified_role: ArcSwapOption<RoleId>,
}

impl InnerCachedChannel {
    fn new(channel: &QueriedChannel) -> Self {
        Self {
            channel: channel.channel,
            guild: channel.guild,
            targets: OnceCell::new(),
            games: OnceCell::new(),
            message: ArcSwapOption::new(channel.message.map(Arc::new)),
            notified_role: ArcSwapOption::new(channel.notified_role.map(Arc::new)),
        }
    }
    const fn id(&self) -> ChannelId {
        self.channel
    }
    const fn guild(&self) -> GuildId {
        self.guild
    }
    fn message(&self) -> Option<MessageId> {
        self.message.load().as_deref().copied()
    }
    fn notified_role(&self) -> Option<RoleId> {
        self.notified_role.load().as_deref().copied()
    }
    async fn get_targets(&self) -> Result<&DashSet<Id, RandomState>, DbErr> {
        self.targets
            .get_or_try_init(|| async { Ok(db().await.get_targets(self.channel).await?.collect()) })
            .await
    }
    async fn get_games(&self) -> Result<&DashSet<Id, RandomState>, DbErr> {
        self.games
            .get_or_try_init(|| async { Ok(db().await.get_games(self.channel).await?.collect()) })
            .await
    }
    async fn add_targets(
        &self,
        targets: impl IntoIterator<Item = Id> + Send,
    ) -> Result<usize, TargetInsertError> {
        let targets = targets.into_iter().collect::<Vec<Id>>();
        let target_count = self.target_count().await?;
        if target_count + targets.len() > GAME_LIMIT {
            return Err(TargetInsertError::LimitExceeded(
                target_count + targets.len(),
            ));
        }
        if targets.is_empty() {
            return Err(TargetInsertError::TargetListEmpty);
        }
        let res = db()
            .await
            .add_targets(self.channel, targets.iter().copied())
            .await?;
        if res != 0 {
            if let Some(targets_map) = self.targets.get() {
                for target in targets {
                    targets_map.insert(target);
                }
            }
            Ok(res)
        } else {
            Err(TargetInsertError::TargetsNotInserted)
        }
    }
    async fn add_games(
        &self,
        games: impl IntoIterator<Item = Id> + Send,
    ) -> Result<usize, GameInsertError> {
        let games = games.into_iter().collect::<Vec<Id>>();
        let game_count = self.game_count().await?;
        if game_count + games.len() > GAME_LIMIT {
            return Err(GameInsertError::LimitExceeded(game_count + games.len()));
        }
        if games.is_empty() {
            return Err(GameInsertError::GameListEmpty);
        }
        let res = db()
            .await
            .add_games(self.channel, games.iter().copied())
            .await?;
        if res != 0 {
            if let Some(games_map) = self.games.get() {
                for game in games {
                    games_map.insert(game);
                }
            }
            Ok(res)
        } else {
            Err(GameInsertError::GamesNotInserted)
        }
    }
    async fn remove_targets(
        &self,
        targets: impl IntoIterator<Item = Id> + Send + Clone,
    ) -> Result<usize, TargetDeleteError> {
        let res = db()
            .await
            .remove_targets(self.channel, targets.clone())
            .await?;
        if res == 0 {
            Err(TargetDeleteError::TargetsNotDeleted)
        } else {
            if let Some(targets_set) = self.targets.get() {
                for target in targets {
                    targets_set.remove(&target);
                }
            }
            Ok(res)
        }
    }
    async fn remove_games(
        &self,
        games: impl IntoIterator<Item = Id> + Send + Clone,
    ) -> Result<usize, GameDeleteError> {
        let res = db().await.remove_games(self.channel, games.clone()).await?;
        if res == 0 {
            Err(GameDeleteError::GamesNotDeleted)
        } else {
            if let Some(games_set) = self.games.get() {
                for game in games {
                    games_set.remove(&game);
                }
            }
            Ok(res)
        }
    }
    async fn clear_targets(&self) -> Result<usize, TargetDeleteError> {
        let res = db().await.clear_targets(self.channel).await?;
        if res == 0 {
            Err(TargetDeleteError::TargetsNotDeleted)
        } else {
            if let Some(targets_set) = self.targets.get() {
                targets_set.clear();
            }
            Ok(res)
        }
    }
    async fn clear_games(&self) -> Result<usize, GameDeleteError> {
        let res = db().await.clear_games(self.channel).await?;
        if res == 0 {
            Err(GameDeleteError::GamesNotDeleted)
        } else {
            if let Some(games_set) = self.games.get() {
                games_set.clear();
            }
            Ok(res)
        }
    }
    async fn game_count(&self) -> Result<usize, DbErr> {
        Ok(self.get_games().await?.len())
    }
    async fn target_count(&self) -> Result<usize, DbErr> {
        Ok(self.get_targets().await?.len())
    }
    async fn set_message(&self, message: MessageId) -> Result<(), DbErr> {
        db().await.set_message(self.channel, message).await?;
        self.message.store(Some(Arc::new(message)));
        Ok(())
    }
    async fn set_notified_role(&self, role: Option<RoleId>) -> Result<(), DbErr> {
        db().await.set_notified_role(self.channel, role).await?;
        self.notified_role.store(role.map(Arc::new));
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ChannelGetError {
    #[error("database error occurred: {0}")]
    Database(#[from] DbErr),
    #[error("The tracker hasn't been initialized in this channel.")]
    NotInitialized,
}

impl From<ChannelGetError> for CommandError {
    fn from(value: ChannelGetError) -> Self {
        match value {
            ChannelGetError::Database(err) => Self::Unexpected(err.into()),
            _ => Self::Expected(value.to_string()),
        }
    }
}

#[derive(Debug, Error)]
pub enum ChannelInitializeError {
    #[error("database error occurred: {0}")]
    Database(DbErr),
    #[error("The tracker has already been initialized in this channel.")]
    AlreadyInitialized,
    #[error("Tracker channel limit exceeded (channels after initializing: {0}/{CHANNEL_LIMIT}).")]
    LimitExceeded(usize),
}

impl From<DbErr> for ChannelInitializeError {
    fn from(value: DbErr) -> Self {
        if value == DbErr::RecordNotInserted {
            Self::AlreadyInitialized
        } else {
            Self::Database(value)
        }
    }
}

impl From<ChannelInitializeError> for CommandError {
    fn from(value: ChannelInitializeError) -> Self {
        match value {
            ChannelInitializeError::Database(err) => Self::Unexpected(err.into()),
            _ => Self::Expected(value.to_string()),
        }
    }
}

#[derive(Debug)]
struct QueriedChannel {
    channel: ChannelId,
    guild: GuildId,
    message: Option<MessageId>,
    notified_role: Option<RoleId>,
}

pub struct Database {
    db: DatabaseConnection,
    channel_cache: Cache<ChannelId, CachedChannel, RandomState>,
    guild_cache: Cache<GuildId, Arc<DashSet<ChannelId>>, RandomState>,
    deleting: DashSet<ChannelId, RandomState>,
}

impl Database {
    async fn new() -> Result<Self, DbErr> {
        let db = sea_orm::Database::connect(DATABASE_URL).await?;
        Migrator::up(&db, None).await?;
        Ok(Self {
            db,
            channel_cache: Cache::builder()
                .max_capacity(2500)
                .build_with_hasher(RandomState::new()),
            guild_cache: Cache::builder()
                .max_capacity(1000)
                .build_with_hasher(RandomState::new()),
            deleting: DashSet::with_hasher(RandomState::new()),
        })
    }
    pub async fn initialize(&self, channel: &GuildChannel) -> Result<(), ChannelInitializeError> {
        let channel_count = self.get_guild_channel_count(channel.guild_id).await?;
        if channel_count >= CHANNEL_LIMIT {
            return Err(ChannelInitializeError::LimitExceeded(channel_count + 1));
        }
        Channel::insert(channel::ActiveModel {
            id: Set(channel.id.get() as i64),
            guild: Set(channel.guild_id.get() as i64),
            message: NotSet,
            notified_role: NotSet,
        })
        .on_conflict(OnConflict::new().do_nothing().to_owned())
        .exec(&self.db)
        .await?;
        let guild_cache = self.guild_cache.get(&channel.guild_id).await;
        if let Some(cache) = guild_cache {
            cache.insert(channel.id);
        }
        self.channel_cache
            .insert(
                channel.id,
                CachedChannel::new(&QueriedChannel {
                    channel: channel.id,
                    guild: channel.guild_id,
                    message: None,
                    notified_role: None,
                }),
            )
            .await;
        Ok(())
    }
    pub async fn get_guild_channels(
        &self,
        guild: GuildId,
    ) -> Result<Arc<DashSet<ChannelId>>, Arc<DbErr>> {
        let res = self
            .guild_cache
            .try_get_with(guild, async {
                Ok(Arc::new(
                    self.query_guild_channels(guild)
                        .await?
                        .collect::<DashSet<ChannelId>>(),
                ))
            })
            .await?;
        Ok(res)
    }
    async fn query_guild_channels(
        &self,
        guild: GuildId,
    ) -> Result<impl Iterator<Item = ChannelId>, DbErr> {
        Ok(Channel::find()
            .select_only()
            .column(channel::Column::Id)
            .filter(channel::Column::Guild.eq(guild.get() as i64))
            .into_tuple::<i64>()
            .all(&self.db)
            .await?
            .into_iter()
            .map(|x| ChannelId::new(x as u64)))
    }
    async fn get_guild_channel_count(&self, guild: GuildId) -> Result<usize, DbErr> {
        let res = match self.guild_cache.get(&guild).await {
            Some(guild) => guild.len(),
            None => {
                Channel::find()
                    .filter(channel::Column::Guild.eq(guild.get() as i64))
                    .into_tuple::<i64>()
                    .count(&self.db)
                    .await? as usize
            }
        };
        Ok(res)
    }
    pub async fn get_channel(
        &self,
        channel: ChannelId,
    ) -> Result<CachedChannel, Arc<ChannelGetError>> {
        if self.deleting.contains(&channel) {
            return Err(Arc::new(ChannelGetError::NotInitialized));
        }
        self.channel_cache
            .try_get_with(channel, async {
                match self.query_channel(channel).await {
                    Ok(channel) => channel.map_or_else(
                        || Err(ChannelGetError::NotInitialized),
                        |channel| Ok(CachedChannel::new(&channel)),
                    ),
                    Err(err) => Err(ChannelGetError::Database(err)),
                }
            })
            .await
    }
    pub async fn get_all_games_and_targets(&self) -> Result<HashMap<Id, Vec<Id>>, DbErr> {
        let mut res: HashMap<Id, Vec<Id>> = HashMap::default();
        Game::find()
            .join(
                JoinType::InnerJoin,
                Game::belongs_to(Target)
                    .from(game::Column::Channel)
                    .to(target::Column::Channel)
                    .into(),
            )
            .select_only()
            .column(game::Column::Id)
            .column(target::Column::Id)
            .distinct()
            .into_tuple::<(i64, i64)>()
            .all(&self.db)
            .await?
            .into_iter()
            .map(|(x, y)| (Id::new(x as u64).unwrap(), Id::new(y as u64).unwrap()))
            .for_each(|(x, y)| {
                res.entry(x).or_default().push(y);
            });
        Ok(res)
    }
    pub async fn get_all_channels(&self) -> Result<impl Iterator<Item = ChannelId>, DbErr> {
        Ok(Channel::find()
            .select_only()
            .column(channel::Column::Id)
            .into_tuple::<i64>()
            .all(&self.db)
            .await?
            .into_iter()
            .map(|x| ChannelId::new(x as u64)))
    }
    async fn get_targets(&self, channel: ChannelId) -> Result<impl Iterator<Item = Id>, DbErr> {
        Ok(Target::find()
            .select_only()
            .column(target::Column::Id)
            .filter(target::Column::Channel.eq(channel.get() as i64))
            .into_tuple::<i64>()
            .all(&self.db)
            .await?
            .into_iter()
            .map(|x| Id::new(x as u64).unwrap()))
    }
    async fn get_games(&self, channel: ChannelId) -> Result<impl Iterator<Item = Id>, DbErr> {
        Ok(Game::find()
            .select_only()
            .column(game::Column::Id)
            .filter(game::Column::Channel.eq(channel.get() as i64))
            .into_tuple::<i64>()
            .all(&self.db)
            .await?
            .into_iter()
            .map(|x| Id::new(x as u64).unwrap()))
    }
    async fn add_targets(
        &self,
        channel: ChannelId,
        targets: impl IntoIterator<Item = Id> + Send,
    ) -> Result<usize, DbErr> {
        let targets = targets.into_iter();
        Ok(
            Target::insert_many(targets.map(|id: Id| target::ActiveModel {
                id: Set(id.get() as i64),
                channel: Set(channel.get() as i64),
            }))
            .on_conflict(OnConflict::new().do_nothing().to_owned())
            .exec_without_returning(&self.db)
            .await? as usize,
        )
    }
    async fn add_games(
        &self,
        channel: ChannelId,
        games: impl IntoIterator<Item = Id> + Send,
    ) -> Result<usize, DbErr> {
        let games = games.into_iter();
        Ok(Game::insert_many(games.map(|id: Id| game::ActiveModel {
            id: Set(id.get() as i64),
            channel: Set(channel.get() as i64),
        }))
        .on_conflict(OnConflict::new().do_nothing().to_owned())
        .exec_without_returning(&self.db)
        .await? as usize)
    }
    async fn remove_targets(
        &self,
        channel: ChannelId,
        targets: impl IntoIterator<Item = Id> + Send,
    ) -> Result<usize, DbErr> {
        Ok(Target::delete_many()
            .filter(target::Column::Id.is_in(targets.into_iter().map(|id| id.get() as i64)))
            .filter(target::Column::Channel.eq(channel.get() as i64))
            .exec(&self.db)
            .await?
            .rows_affected as usize)
    }
    async fn remove_games(
        &self,
        channel: ChannelId,
        games: impl IntoIterator<Item = Id> + Send,
    ) -> Result<usize, DbErr> {
        Ok(Game::delete_many()
            .filter(game::Column::Id.is_in(games.into_iter().map(|id| id.get() as i64)))
            .filter(game::Column::Channel.eq(channel.get() as i64))
            .exec(&self.db)
            .await?
            .rows_affected as usize)
    }
    async fn clear_targets(&self, channel: ChannelId) -> Result<usize, DbErr> {
        Ok(Target::delete_many()
            .filter(target::Column::Channel.eq(channel.get() as i64))
            .exec(&self.db)
            .await?
            .rows_affected as usize)
    }
    async fn clear_games(&self, channel: ChannelId) -> Result<usize, DbErr> {
        Ok(Game::delete_many()
            .filter(game::Column::Channel.eq(channel.get() as i64))
            .exec(&self.db)
            .await?
            .rows_affected as usize)
    }
    async fn set_notified_role(
        &self,
        channel: ChannelId,
        role: Option<RoleId>,
    ) -> Result<(), DbErr> {
        Channel::update(channel::ActiveModel {
            id: Set(channel.get() as i64),
            guild: NotSet,
            message: NotSet,
            notified_role: Set(role.map(|role| role.get() as i64)),
        })
        .exec(&self.db)
        .await?;
        Ok(())
    }
    async fn set_message(&self, channel: ChannelId, message: MessageId) -> Result<(), DbErr> {
        Channel::update(channel::ActiveModel {
            id: Set(channel.get() as i64),
            guild: NotSet,
            message: Set(Some(message.get() as i64)),
            notified_role: NotSet,
        })
        .exec(&self.db)
        .await?;
        Ok(())
    }
    async fn delete_channel(
        &self,
        channel: Arc<InnerCachedChannel>,
    ) -> Result<(), ChannelDeleteError> {
        let channel_id = channel.id();
        let guild_id = channel.guild();
        self.deleting.insert(channel_id);
        self.channel_cache.invalidate(&channel_id).await;
        self.channel_cache.run_pending_tasks().await;
        let res = {
            match Arc::into_inner(channel) {
                None => Err(ChannelDeleteError::OperationPending),
                Some(_) => {
                    Channel::delete_by_id(channel_id.get() as i64)
                        .exec(&self.db)
                        .await?;
                    if let Some(guild) = self.guild_cache.get(&guild_id).await {
                        guild.remove(&channel_id);
                    }
                    Ok(())
                }
            }
        };
        self.deleting.remove(&channel_id);
        res
    }
    async fn query_channel(&self, channel: ChannelId) -> Result<Option<QueriedChannel>, DbErr> {
        Ok(Channel::find_by_id(channel.get() as i64)
            .select_only()
            .column(channel::Column::Guild)
            .column(channel::Column::Message)
            .column(channel::Column::NotifiedRole)
            .into_tuple::<(i64, Option<i64>, Option<i64>)>()
            .one(&self.db)
            .await?
            .map(|res| {
                let (guild_id, message_id, notified_role_id) = res;
                QueriedChannel {
                    channel,
                    guild: GuildId::new(guild_id as u64),
                    message: message_id.map(|id| MessageId::new(id as u64)),
                    notified_role: notified_role_id.map(|id| RoleId::new(id as u64)),
                }
            }))
    }
    pub async fn get_game_count(&self) -> Result<u64, DbErr> {
        Game::find()
            .select_only()
            .column(game::Column::Id)
            .distinct()
            .count(&self.db)
            .await
    }
    pub async fn get_target_count(&self) -> Result<u64, DbErr> {
        Target::find()
            .select_only()
            .column(target::Column::Id)
            .distinct()
            .count(&self.db)
            .await
    }
}
