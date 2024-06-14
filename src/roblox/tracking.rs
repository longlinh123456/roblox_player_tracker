use super::{
    clear_thumbnail_cache, client, get_thumbnail_from_token, retry_strategy,
    thumbnail_retry_strategy, InfiniteRetry, ThumbnailError,
};
use crate::{
    commands::stats::get_stats,
    constants::{MAX_TRACKING_TASKS, MIN_TRACKING_DELAY, MISSING_TARGET_TOLERANCE},
    database::db,
    roblox::get_thumbnail_from_user_id,
};
use ahash::{HashMap, HashMapExt, HashSet, HashSetExt, RandomState};
use backon::Retryable;
use batch_aint_one::BatchError;
use dashmap::{DashMap, DashSet};
use poise::serenity_prelude::futures::{
    future,
    stream::{self, FuturesUnordered},
    StreamExt,
};
use roblox_api::apis::{
    self,
    games::{GamesApi, PublicServer, ServerType},
    thumbnails::ThumbnailErrorState,
    Error, Id, JsonError, Paginator, RequestLimit, SortOrder,
};
use sea_orm::prelude::Uuid;
use std::{
    collections::hash_map::Entry,
    sync::{Arc, OnceLock},
};
use tokio::time::{self, Instant};

fn get_servers(game_id: Id) -> Paginator<'static, PublicServer, JsonError> {
    apis::paginate(
        move |cursor| async move {
            (|| async {
                client()
                    .get_public_servers_manual(
                        game_id,
                        ServerType::Public,
                        SortOrder::Descending,
                        false,
                        RequestLimit::OneHundred,
                        cursor.as_deref(),
                    )
                    .await
            })
            .retry(retry_strategy())
            .when(api_error_retryable)
            .await
        },
        None::<String>,
    )
}

const fn api_error_retryable(err: &Error<JsonError>) -> bool {
    matches!(*err, Error::RateLimit | Error::Request(_))
}

fn thumbnail_error_retryable(err: &ThumbnailError) -> bool {
    match *err {
        ThumbnailError::Batch(ref err) => {
            if let BatchError::BatchFailed(err) = err {
                matches!(**err, Error::RateLimit | Error::Request(_))
            } else {
                false
            }
        }
        ThumbnailError::Thumbnail(ref err) => {
            matches!(
                err.state,
                ThumbnailErrorState::TemporarilyUnavailable | ThumbnailErrorState::Pending
            )
        }
    }
}

#[derive(Debug, Clone)]
pub struct TargetState {
    pub game: Id,
    pub server: Uuid,
}

static TARGET_STATES: OnceLock<DashMap<Id, TargetState, RandomState>> = OnceLock::new();

pub fn target_states() -> &'static DashMap<Id, TargetState, RandomState> {
    TARGET_STATES.get_or_init(DashMap::default)
}

struct ServerPlayer {
    pub game: Id,
    pub server: Uuid,
    pub token: String,
}

fn target_states_cleanup(
    games_and_targets: &HashMap<Id, Vec<Id>>,
    found_targets: &DashSet<Id, RandomState>,
    missing_targets: &mut HashMap<Id, usize>,
) {
    let mut all_targets: HashSet<Id> = HashSet::new();
    for targets in games_and_targets.values() {
        for target in targets {
            all_targets.insert(*target);
        }
    }
    missing_targets
        .retain(|target, _| !found_targets.contains(target) && all_targets.contains(target));
    for target in &all_targets {
        if !found_targets.contains(target) {
            *missing_targets.entry(*target).or_default() += 1;
        }
    }
    target_states().retain(|id, _| {
        all_targets.contains(id) && {
            match missing_targets.entry(*id) {
                Entry::Vacant(_) => true,
                Entry::Occupied(entry) => {
                    if *entry.get() > MISSING_TARGET_TOLERANCE {
                        entry.remove();
                        false
                    } else {
                        true
                    }
                }
            }
        }
    });
}

async fn get_target_thumbnails(
    games_and_targets: &HashMap<Id, Vec<Id>>,
) -> HashMap<Id, HashMap<String, Id>> {
    let mut target_thumbnails: HashMap<Id, HashMap<String, Id>> = HashMap::new();
    for (game, targets) in games_and_targets {
        let thumbnails = targets
            .iter()
            .map(|id| async move {
                (
                    (|| get_thumbnail_from_user_id(*id))
                        .retry(thumbnail_retry_strategy())
                        .when(|err| thumbnail_error_retryable(err))
                        .await,
                    id,
                )
            })
            .collect::<FuturesUnordered<_>>()
            .filter_map(|(res, id)| future::ready(res.map_or(None, |res| Some((res, *id)))))
            .collect::<HashMap<String, Id>>()
            .await;
        if thumbnails.is_empty() {
            continue;
        }
        target_thumbnails.insert(*game, thumbnails);
    }
    target_thumbnails
}

pub async fn tracking_loop() {
    let mut missing_targets: HashMap<Id, usize> = HashMap::default();
    loop {
        let start_time = Instant::now();
        clear_thumbnail_cache().await;
        let games_and_targets = (|| async { db().await.get_all_games_and_targets().await })
            .retry(&InfiniteRetry)
            .await
            .unwrap();
        let target_thumbnails = Arc::new(get_target_thumbnails(&games_and_targets).await);
        let found_targets: Arc<DashSet<Id, RandomState>> = Arc::default();
        stream::iter(target_thumbnails.keys().copied().map(|game| {
            get_servers(game)
                .take_while(|res| future::ready(res.is_ok()))
                .map(move |res| {
                    stream::iter(res.unwrap().data.into_iter().flat_map(move |server| {
                        server
                            .player_tokens
                            .into_iter()
                            .map(move |token| ServerPlayer {
                                server: server.id,
                                game,
                                token,
                            })
                    }))
                })
                .flatten_unordered(None)
        }))
        .flatten_unordered(MAX_TRACKING_TASKS)
        .for_each_concurrent(None, |server_player| {
            let target_thumbnails = target_thumbnails.clone();
            let found_targets = found_targets.clone();
            async move {
                if let Some(target_thumbnails) = target_thumbnails.get(&server_player.game) {
                    let thumbnail = (|| get_thumbnail_from_token(&server_player.token))
                        .retry(thumbnail_retry_strategy())
                        .when(|err| thumbnail_error_retryable(err))
                        .await;
                    if let Ok(thumbnail) = thumbnail {
                        if let Some(target) = target_thumbnails.get(&thumbnail) {
                            target_states().insert(
                                *target,
                                TargetState {
                                    server: server_player.server,
                                    game: server_player.game,
                                },
                            );
                            found_targets.insert(*target);
                        }
                    }
                }
            }
        })
        .await;
        target_states_cleanup(&games_and_targets, &found_targets, &mut missing_targets);
        time::sleep_until(start_time + MIN_TRACKING_DELAY).await;
        get_stats().add_tracking_cycle(start_time.elapsed());
    }
}
