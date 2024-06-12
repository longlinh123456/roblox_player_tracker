use std::{sync::Arc, time::Instant};

use ahash::{HashMap, HashSet, RandomState};
use backon::Retryable;
use dashmap::{DashMap, DashSet};
use poise::serenity_prelude::{
    futures::{
        stream::{self, FuturesUnordered},
        StreamExt,
    },
    Cache, ChannelId, CreateMessage, EditMessage, Error as SerenityError, Http, HttpError, Mention,
    MessageId, RoleId,
};
use roblox_api::apis::Id;
use tokio::time;

use crate::{
    commands::stats::get_stats,
    constants::UPDATE_DELAY,
    database::db,
    message_utils::{render_lines_edit_message, render_lines_message},
};

use super::{
    get_game_name, get_username, retry_strategy,
    tracking::{target_states, TargetState},
    InfiniteRetry,
};

fn is_different_states(
    old_state: Option<&TargetState>,
    current_state: Option<&TargetState>,
    games: &DashSet<Id, RandomState>,
) -> bool {
    if let Some(current_state) = current_state {
        if !games.contains(&current_state.game) {
            return false;
        }
        if let Some(old_state) = old_state {
            if current_state.server != old_state.server {
                return true;
            }
        } else {
            return true;
        }
    }
    false
}

const fn should_retry_send(err: &SerenityError) -> bool {
    if let SerenityError::Http(HttpError::UnsuccessfulRequest(err)) = err {
        if let 10003 | 50001 = err.error.code {
            return false;
        }
    }
    true
}
const fn should_retry_edit(err: &SerenityError) -> bool {
    if let SerenityError::Http(HttpError::UnsuccessfulRequest(err)) = err {
        if let 10003 | 10008 | 50005 | 50001 = err.error.code {
            return false;
        }
    }
    true
}
const fn should_send_message(err: &SerenityError) -> bool {
    if let SerenityError::Http(HttpError::UnsuccessfulRequest(err)) = err {
        if let 10008 | 50005 = err.error.code {
            return true;
        }
    }
    false
}
const fn should_delete_tracker(err: &SerenityError) -> bool {
    if let SerenityError::Http(HttpError::UnsuccessfulRequest(err)) = err {
        if err.error.code == 10003 {
            return true;
        }
    }
    false
}

async fn generate_tracking_output(
    channel_state: &HashMap<Id, TargetState>,
    channel: ChannelId,
    notified_role: Option<RoleId>,
) -> (CreateMessage, EditMessage) {
    let lines = channel_state
        .iter()
        .map(|(id, state)| async {
            format!(
                "{}: [{}](http://www.roblox.com/home?placeId={}&gameId={})",
                get_username(*id).await,
                get_game_name(state.game).await,
                state.game,
                state.server
            )
        })
        .collect::<FuturesUnordered<_>>()
        .collect::<Vec<String>>()
        .await;
    let title = format!("Tracking output for channel {}:", Mention::Channel(channel));
    let content = notified_role.map_or_else(String::new, |notified_role| {
        Mention::Role(notified_role).to_string()
    });
    (
        render_lines_message(&content, &lines, &title),
        render_lines_edit_message(content, lines, title),
    )
}
async fn send_output(
    cache: &Arc<Cache>,
    http: &Http,
    output: CreateMessage,
    edit_output: EditMessage,
    message_id: Option<MessageId>,
    channel_id: ChannelId,
) {
    let mut should_send = false;
    let mut should_delete = false;
    if let Some(message_id) = message_id {
        let edit_res = (|| http.edit_message(channel_id, message_id, &edit_output, Vec::new()))
            .retry(retry_strategy())
            .when(should_retry_edit)
            .await;
        if let Err(err) = edit_res {
            should_send = should_send_message(&err);
            should_delete = should_delete_tracker(&err);
        }
    }
    if should_delete {
        let channel = (|| async { db().await.get_channel(channel_id).await })
            .retry(retry_strategy())
            .await;
        if let Ok(channel) = channel {
            let _ = channel.delete_channel().await;
        }
    } else if should_send || message_id.is_none() {
        let send_res = (|| channel_id.send_message((cache, http), output.clone()))
            .retry(retry_strategy())
            .when(should_retry_send)
            .await;
        if let Ok(send_res) = send_res {
            let channel = (|| async { db().await.get_channel(channel_id).await })
                .retry(retry_strategy())
                .await;
            if let Ok(channel) = channel {
                let _ = (|| channel.set_message(send_res.id))
                    .retry(retry_strategy())
                    .await;
            }
        }
    }
}

pub async fn update_loop(cache: Arc<Cache>, http: Arc<Http>) {
    let channel_states: Arc<DashMap<ChannelId, HashMap<Id, TargetState>, RandomState>> =
        Arc::default();
    loop {
        let start_time = Instant::now();
        let channel_ids = (|| async { db().await.get_all_channels().await })
            .retry(&InfiniteRetry)
            .await
            .unwrap()
            .collect::<HashSet<ChannelId>>();
        channel_states.retain(|id, _| channel_ids.contains(id));
        stream::iter(channel_ids)
            .for_each_concurrent(None, |channel_id| {
                let channel_states = channel_states.clone();
                let cache = cache.clone();
                let http = http.clone();
                async move {
                    let channel = (|| async { db().await.get_channel(channel_id).await })
                        .retry(retry_strategy())
                        .await;
                    if let Ok(channel) = channel {
                        let games = (|| channel.get_games()).retry(retry_strategy()).await;
                        let targets = (|| channel.get_targets()).retry(retry_strategy()).await;
                        let notified_role = channel.notified_role();
                        let message_id = channel.message();
                        if let Ok(games) = games {
                            if let Ok(targets) = targets {
                                let mut channel_state =
                                    channel_states.entry(channel_id).or_default();
                                channel_state.retain(|target, _| targets.contains(target));
                                let mut ping = false;
                                for target in targets.iter() {
                                    let current_state_ref = target_states().get(target.as_ref());
                                    let current_state = current_state_ref.as_deref();
                                    let old_state = channel_state.get(target.as_ref());
                                    if !ping {
                                        ping = is_different_states(old_state, current_state, games);
                                    }
                                    match current_state {
                                        Some(state) if games.contains(&state.game) => {
                                            channel_state.insert(*target, state.clone());
                                            drop(current_state_ref);
                                        }
                                        _ => {
                                            drop(current_state_ref);
                                            channel_state.remove(target.as_ref());
                                        }
                                    };
                                }
                                drop(channel);
                                let channel_state = {
                                    let copied = channel_state.value().clone();
                                    drop(channel_state);
                                    copied
                                };
                                let (output, edit_output) = generate_tracking_output(
                                    &channel_state,
                                    channel_id,
                                    if ping { notified_role } else { None },
                                )
                                .await;
                                send_output(
                                    &cache,
                                    http.as_ref(),
                                    output,
                                    edit_output,
                                    message_id,
                                    channel_id,
                                )
                                .await;
                            }
                        }
                    }
                }
            })
            .await;
        time::sleep(UPDATE_DELAY).await;
        get_stats().add_update_cycle(start_time.elapsed());
    }
}