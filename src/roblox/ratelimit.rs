use leaky_bucket::RateLimiter as InnerRateLimiter;
use std::{sync::OnceLock, time::Duration};

pub(super) struct RateLimiter {
    pub(super) thumbnails: InnerRateLimiter,
    pub(super) servers: InnerRateLimiter,
}
static RATELIMITER: OnceLock<RateLimiter> = OnceLock::new();

pub(super) fn ratelimiter() -> &'static RateLimiter {
    RATELIMITER.get_or_init(|| RateLimiter {
        thumbnails: InnerRateLimiter::builder()
            .interval(Duration::from_millis(1500))
            .refill(50)
            .max(50)
            .initial(50)
            .build(),
        servers: InnerRateLimiter::builder()
            .interval(Duration::from_millis(3500))
            .refill(10)
            .max(10)
            .initial(10)
            .build(),
    })
}
