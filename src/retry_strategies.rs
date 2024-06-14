use std::{sync::OnceLock, time::Duration};

use backon::FibonacciBuilder;

static ROBLOX_RETRY_STRATEGY: OnceLock<FibonacciBuilder> = OnceLock::new();

pub fn roblox_retry_strategy() -> &'static FibonacciBuilder {
    ROBLOX_RETRY_STRATEGY.get_or_init(|| {
        FibonacciBuilder::default()
            .with_jitter()
            .with_min_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_millis(3000))
            .with_max_times(15)
    })
}

static THUMBNAIL_RETRY_STRATEGY: OnceLock<FibonacciBuilder> = OnceLock::new();

pub fn thumbnail_retry_strategy() -> &'static FibonacciBuilder {
    THUMBNAIL_RETRY_STRATEGY.get_or_init(|| {
        FibonacciBuilder::default()
            .with_jitter()
            .with_min_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_millis(3000))
            .with_max_times(15 + 1)
    })
}

static DISCORD_RETRY_STRATEGY: OnceLock<FibonacciBuilder> = OnceLock::new();

pub fn discord_retry_strategy() -> &'static FibonacciBuilder {
    DISCORD_RETRY_STRATEGY.get_or_init(|| {
        FibonacciBuilder::default()
            .with_jitter()
            .with_min_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_millis(500))
            .with_max_times(5)
    })
}
