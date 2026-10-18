use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::SystemTime;

use crate::error::AppError;

pub fn init(log_level: String) -> Result<(), AppError> {
    let level_filter = log_level
        .parse::<LevelFilter>()
        .unwrap_or(LevelFilter::INFO);

    let filter = EnvFilter::builder()
        .with_default_directive(level_filter.into())
        .from_env_lossy();

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_timer(SystemTime)
        .try_init()
        .map_err(|e| AppError::StartupFailed("init stdout logger", e.to_string()))?;

    Ok(())
}
