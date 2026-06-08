use crate::settings::LogProfile;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

pub fn init_logging(profile: LogProfile) {
    let level = match profile {
        LogProfile::Product => LevelFilter::INFO,
        LogProfile::FieldDebug => LevelFilter::DEBUG,
        LogProfile::Development => LevelFilter::TRACE,
    };

    let subscriber = tracing_subscriber::registry().with(fmt::layer().with_filter(level));
    let _ = tracing::subscriber::set_global_default(subscriber);
}
