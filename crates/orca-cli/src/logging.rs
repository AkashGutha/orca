use std::sync::OnceLock;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;

use orca_core::errors::AppError;

static LOGGING_INITIALIZED: OnceLock<()> = OnceLock::new();

pub fn init_logging(json: bool) -> Result<(), AppError> {
    if LOGGING_INITIALIZED.get().is_some() {
        return Ok(());
    }

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    let result = if json {
        fmt()
            .json()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .try_init()
    } else {
        fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .without_time()
            .try_init()
    };

    result.map_err(|err| AppError::Logging(err.to_string()))?;
    let _ = LOGGING_INITIALIZED.set(());
    Ok(())
}
