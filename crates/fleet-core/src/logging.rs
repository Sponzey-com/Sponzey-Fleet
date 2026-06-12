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

    let subscriber = tracing_subscriber::registry().with(
        fmt::layer()
            .compact()
            .with_level(true)
            .with_target(true)
            .with_writer(std::io::stderr)
            .with_filter(level),
    );
    let _ = tracing::subscriber::set_global_default(subscriber);
}

pub fn format_warning_message(message: impl AsRef<str>) -> String {
    format!("WARNING: {}", message.as_ref())
}

pub fn format_error_message(message: impl AsRef<str>) -> String {
    format!("ERROR: {}", message.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_prefixes_are_uppercase_and_easy_to_scan() {
        assert_eq!(format_warning_message("check TLS"), "WARNING: check TLS");
        assert_eq!(format_error_message("store failed"), "ERROR: store failed");
    }
}
