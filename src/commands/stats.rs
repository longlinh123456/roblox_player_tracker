use std::{
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use ahash::RandomState;
use moka::future::Cache;
use poise::{command, serenity_prelude::futures::TryFutureExt, CreateReply};
use sea_orm::DbErr;
use simple_moving_average::{NoSumSMA, SMA};

use crate::{
    commands::{CommandResult, Context},
    database::db,
    message_utils::info_embed,
};

#[derive(Debug)]
pub struct Stats {
    game_count: Cache<(), u64, RandomState>,
    target_count: Cache<(), u64, RandomState>,
    secs_per_tracking_cycle: Mutex<NoSumSMA<Duration, u32, 10>>,
    secs_per_update_cycle: Mutex<NoSumSMA<Duration, u32, 10>>,
}

impl Stats {
    fn new() -> Self {
        Self {
            game_count: Cache::builder()
                .max_capacity(1)
                .initial_capacity(1)
                .time_to_live(Duration::from_secs(60))
                .build_with_hasher(RandomState::new()),
            target_count: Cache::builder()
                .max_capacity(1)
                .initial_capacity(1)
                .time_to_live(Duration::from_secs(60))
                .build_with_hasher(RandomState::new()),
            secs_per_tracking_cycle: Mutex::new(NoSumSMA::from_zero(Duration::ZERO)),
            secs_per_update_cycle: Mutex::new(NoSumSMA::from_zero(Duration::ZERO)),
        }
    }
    async fn game_count(&self) -> Result<u64, Arc<DbErr>> {
        self.game_count
            .try_get_with((), async { db().await.get_game_count().await })
            .await
    }
    async fn target_count(&self) -> Result<u64, Arc<DbErr>> {
        self.target_count
            .try_get_with((), async { db().await.get_target_count().await })
            .await
    }
    pub fn secs_per_tracking_cycle(&self) -> Duration {
        self.secs_per_tracking_cycle.lock().unwrap().get_average()
    }
    pub fn secs_per_update_cycle(&self) -> Duration {
        self.secs_per_update_cycle.lock().unwrap().get_average()
    }
    pub fn add_tracking_cycle(&self, cycle: Duration) {
        self.secs_per_tracking_cycle
            .lock()
            .unwrap()
            .add_sample(cycle);
    }
    pub fn add_update_cycle(&self, cycle: Duration) {
        self.secs_per_update_cycle.lock().unwrap().add_sample(cycle);
    }
}
impl Default for Stats {
    fn default() -> Self {
        Self::new()
    }
}

static STATS: OnceLock<Stats> = OnceLock::new();

pub fn get_stats() -> &'static Stats {
    STATS.get_or_init(Stats::new)
}

/// Get global stats for the tracker
#[command(slash_command, ephemeral)]
pub async fn stats(ctx: Context<'_>) -> CommandResult {
    ctx.send(CreateReply::default().embed(
        info_embed(
            format!(
                "Game count: {}\nTarget count: {}\nSeconds per tracking cycle: {:.2}\nSeconds per update cycle: {:.2}",
                get_stats().game_count().map_ok_or_else(
                    |_| String::from("failed to get"),
                    |count| ToString::to_string(&count)).await,
                get_stats().target_count().map_ok_or_else(
                    |_| String::from("failed to get"),
                    |count| ToString::to_string(&count)).await,
                get_stats().secs_per_tracking_cycle().as_secs_f32(),
                get_stats().secs_per_update_cycle().as_secs_f32()
            )
        )
        .title("Tracker stats")
    ))
    .await?;
    Ok(())
}
