use crate::constants::{NAME_BATCHING_TIME, NAME_TIMEOUT, THUMBNAIL_BATCHING_TIME, USER_AGENT};
use ahash::{HashMap, RandomState};
use backon::BackoffBuilder;
use batch_aint_one::{
    BatchError, Batcher as InnerBatcher, BatchingPolicy, Limits, OnFull, Processor,
};
use migration::async_trait::async_trait;
use moka::future::Cache;
use poise::serenity_prelude::futures::{future, TryFutureExt};
use roblox_api::{
    apis::{
        self,
        games::GamesApi,
        thumbnails::{
            BatchRequest, BatchThumbnail, BatchThumbnailError, BatchThumbnailResult,
            BatchThumbnailResultExt, ThumbnailFormat, ThumbnailSize, ThumbnailType, ThumbnailsApi,
        },
        users::UsersApi,
        Id, JsonError, OptionId, RequestResult, StringError,
    },
    clients::{Client, ClientBuilder},
};
use std::{
    convert::Infallible,
    iter::{self, Repeat},
    sync::{Arc, OnceLock},
    time::Duration,
};
use thiserror::Error;
use tokio::{sync::OnceCell, task, time};

pub mod tracking;
pub mod update;

#[derive(Debug)]
struct RobloxCache {
    username: Cache<Id, String, RandomState>,
    game_name: Cache<Id, String, RandomState>,
    thumbnail_from_token: Cache<String, String, RandomState>,
    thumbnail_from_user_id: Cache<Id, String, RandomState>,
}
type UsernameBatcher = InnerBatcher<(), Id, String, Infallible>;
type ThumbnailBatcher =
    InnerBatcher<(), ThumbnailRequest, BatchThumbnailResult, Arc<apis::Error<JsonError>>>;
#[derive(Debug, Default, Clone)]
struct InfiniteRetry;

impl BackoffBuilder for InfiniteRetry {
    type Backoff = Repeat<Duration>;
    fn build(&self) -> Self::Backoff {
        iter::repeat(Duration::from_secs(1))
    }
}

#[derive(Debug)]
struct Batcher {
    username: UsernameBatcher,
    thumbnail: ThumbnailBatcher,
}

#[derive(Debug)]
enum ThumbnailRequest {
    User(Id),
    Token(String),
}

#[derive(Debug, Clone)]
struct ThumbnailProcessor;
#[async_trait]
impl Processor<(), ThumbnailRequest, BatchThumbnailResult, Arc<apis::Error<JsonError>>>
    for ThumbnailProcessor
{
    async fn process(
        &self,
        _key: (),
        inputs: impl Iterator<Item = ThumbnailRequest> + Send,
    ) -> Result<Vec<BatchThumbnailResult>, Arc<apis::Error<JsonError>>> {
        let ids_and_tokens = inputs.collect::<Vec<ThumbnailRequest>>();
        let requests = ids_and_tokens
            .iter()
            .enumerate()
            .map(|(index, request)| BatchRequest {
                request_id: Some(index),
                target_id: if let ThumbnailRequest::User(id) = request {
                    OptionId::Some(*id)
                } else {
                    OptionId::None
                },
                token: if let ThumbnailRequest::Token(token) = request {
                    Some(token)
                } else {
                    None
                },
                alias: None::<()>,
                r#type: ThumbnailType::AvatarHeadShot,
                size: ThumbnailSize::_48x48,
                format: ThumbnailFormat::Png,
                circular: false,
            });
        let mut res = Vec::with_capacity(ids_and_tokens.len());
        res.resize_with(ids_and_tokens.len(), || Ok(BatchThumbnail::default()));
        client()
            .get_batch_thumbnails(requests)
            .await?
            .into_iter()
            .for_each(|thumbnail| {
                let index = thumbnail.request_id().unwrap().parse::<usize>().unwrap();
                res[index] = thumbnail;
            });
        Ok(res)
    }
}

#[derive(Debug, Clone)]
struct UsernameProcessor;
#[async_trait]
impl Processor<(), Id, String, Infallible> for UsernameProcessor {
    async fn process(
        &self,
        _key: (),
        inputs: impl Iterator<Item = Id> + Send,
    ) -> Result<Vec<String>, Infallible> {
        let users = inputs.collect::<Vec<Id>>();
        let res = client()
            .get_user_info_from_id_batch(users.iter().copied(), false)
            .await;
        Ok(match res {
            Ok(res) => {
                let res = res
                    .into_iter()
                    .map(|info| (info.id, info.name))
                    .collect::<HashMap<Id, String>>();
                users
                    .into_iter()
                    .map(|id| {
                        res.get(&id)
                            .map_or_else(|| format!("{id} (id)"), Clone::clone)
                    })
                    .collect()
            }
            Err(_) => users.into_iter().map(|id| format!("{id} (id)")).collect(),
        })
    }
}
static CACHE: OnceCell<RobloxCache> = OnceCell::const_new();
static CLIENT: OnceLock<Client> = OnceLock::new();
static BATCHER: OnceLock<Batcher> = OnceLock::new();

async fn cache() -> &'static RobloxCache {
    CACHE
        .get_or_init(|| {
            future::ready(RobloxCache {
                username: Cache::builder()
                    .max_capacity(100000)
                    .time_to_live(Duration::from_secs(60 * 60 * 24))
                    .build_with_hasher(RandomState::new()),
                game_name: Cache::builder()
                    .max_capacity(100000)
                    .time_to_live(Duration::from_secs(60 * 60 * 24))
                    .build_with_hasher(RandomState::new()),
                thumbnail_from_token: Cache::builder()
                    .max_capacity(100000)
                    .build_with_hasher(RandomState::new()),
                thumbnail_from_user_id: Cache::builder()
                    .max_capacity(100000)
                    .build_with_hasher(RandomState::new()),
            })
        })
        .await
}
fn client() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::new(
            ClientBuilder::new()
                .no_proxy()
                .http2_prior_knowledge()
                .user_agent(USER_AGENT),
        )
    })
}
fn batcher() -> &'static Batcher {
    BATCHER.get_or_init(|| Batcher {
        username: InnerBatcher::new(
            UsernameProcessor,
            Limits::default()
                .max_batch_size(200)
                .max_key_concurrency(usize::MAX),
            BatchingPolicy::Duration(NAME_BATCHING_TIME, OnFull::Process),
        ),
        thumbnail: InnerBatcher::new(
            ThumbnailProcessor,
            Limits::default()
                .max_batch_size(100)
                .max_key_concurrency(usize::MAX),
            BatchingPolicy::Duration(THUMBNAIL_BATCHING_TIME, OnFull::Process),
        ),
    })
}

async fn request_game_name(game: Id) -> RequestResult<String, StringError> {
    Ok(client().get_place_details(game).await?.name)
}

pub async fn get_game_name(game: Id) -> String {
    let mut request = Box::pin(
        cache()
            .await
            .game_name
            .try_get_with(game, request_game_name(game))
            .unwrap_or_else(move |_| format!("{game} (id)")),
    );
    time::timeout(NAME_TIMEOUT, &mut request)
        .await
        .unwrap_or_else(|_| {
            task::spawn(request);
            format!("{game} (id)")
        })
}

pub async fn get_username(user: Id) -> String {
    let mut request = Box::pin(cache().await.username.get_with(user, async move {
        batcher().username.add((), user).await.unwrap()
    }));
    time::timeout(NAME_TIMEOUT, &mut request)
        .await
        .unwrap_or_else(|_| {
            task::spawn(request);
            format!("{user} (id)")
        })
}

#[derive(Debug, Error)]
pub enum ThumbnailError {
    #[error(transparent)]
    Batch(#[from] BatchError<Arc<apis::Error<JsonError>>>),
    #[error("thumbnail request failed: {self:?}")]
    Thumbnail(#[from] BatchThumbnailError),
}

pub async fn clear_thumbnail_cache() {
    cache().await.thumbnail_from_token.invalidate_all();
    cache().await.thumbnail_from_user_id.invalidate_all();
}

pub async fn get_thumbnail_from_token(
    token: impl Into<String> + Send,
) -> Result<String, Arc<ThumbnailError>> {
    let token: String = token.into();
    cache()
        .await
        .thumbnail_from_token
        .try_get_with_by_ref(&token, async {
            Ok(batcher()
                .thumbnail
                .add((), ThumbnailRequest::Token(token.clone()))
                .await??
                .image_url)
        })
        .await
}
pub async fn get_thumbnail_from_user_id(user_id: Id) -> Result<String, Arc<ThumbnailError>> {
    cache()
        .await
        .thumbnail_from_user_id
        .try_get_with(user_id, async {
            Ok(batcher()
                .thumbnail
                .add((), ThumbnailRequest::User(user_id))
                .await??
                .image_url)
        })
        .await
}
