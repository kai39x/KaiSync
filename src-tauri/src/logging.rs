//! Rolling file log in `%APPDATA%\KaiSync\logs\` (daily files, last 7 kept).

use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::settings::app_dir;

pub fn init() -> Option<WorkerGuard> {
    let dir = app_dir().join("logs");
    let _ = std::fs::create_dir_all(&dir);

    let filter = EnvFilter::try_from_env("KAISYNC_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info,kaisync_lib=debug"));

    let appender = rolling::Builder::new()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix("kaisync")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&dir);

    let (file_layer, guard) = match appender {
        Ok(app) => {
            let (nb, guard) = tracing_appender::non_blocking(app);
            (Some(fmt::layer().with_writer(nb).with_ansi(false).with_target(false)), Some(guard))
        }
        Err(e) => {
            eprintln!("KaiSync: cannot open log dir {}: {e}", dir.display());
            (None, None)
        }
    };

    let console = cfg!(debug_assertions).then(|| fmt::layer().with_target(false));

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(console)
        .init();

    tracing::info!("KaiSync {} starting, logs in {}", env!("CARGO_PKG_VERSION"), dir.display());
    guard
}
