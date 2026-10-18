#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Network IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Fatal startup failure during {0}: {1}")]
    StartupFailed(&'static str, String),

    #[error("An unexpected error occurred: {0}")]
    Other(String),
}
