use poise::serenity_prelude::Colour;
use std::time::Duration;

pub const DATABASE_URL: &str = "sqlite:./tracker.sqlite?mode=rwc";
pub const SUCCESS_COLOR: Colour = Colour::from_rgb(40, 167, 69);
pub const FAILURE_COLOR: Colour = Colour::from_rgb(231, 76, 60);
pub const INFO_COLOR: Colour = Colour::from_rgb(35, 127, 235);
pub const CHANNEL_LIMIT: usize = 5;
pub const TARGET_LIMIT: usize = 100;
pub const GAME_LIMIT: usize = 100;
pub const DESCRIPTION_MAX_LENGTH: usize = 4096;
pub const NAME_TIMEOUT: Duration = Duration::from_millis(2000);
pub const NAME_BATCHING_TIME: Duration = Duration::from_millis(100);
pub const THUMBNAIL_BATCHING_TIME: Duration = Duration::from_millis(100);
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/96.0.4664.110 Safari/537.36";
pub const MIN_UPDATE_DELAY: Duration = Duration::from_secs(1);
pub const MIN_TRACKING_DELAY: Duration = Duration::from_secs(1);
pub const MAX_TRACKING_TASKS: usize = 3;
pub const MISSING_TARGET_TOLERANCE: usize = 3;
