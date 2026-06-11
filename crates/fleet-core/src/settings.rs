use std::fmt::{Display, Formatter};
use std::net::SocketAddr;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogProfile {
    #[default]
    Product,
    FieldDebug,
    Development,
}

impl FromStr for LogProfile {
    type Err = SettingsError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "product" | "Product" => Ok(Self::Product),
            "field-debug" | "field_debug" | "FieldDebug" => Ok(Self::FieldDebug),
            "development" | "dev" | "Development" => Ok(Self::Development),
            _ => Err(SettingsError::InvalidLogProfile(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub bind_addr: SocketAddr,
    pub controller_url: Option<String>,
    pub log_profile: LogProfile,
}

impl Settings {
    pub fn new(
        bind_addr: SocketAddr,
        controller_url: Option<String>,
        log_profile: LogProfile,
    ) -> Result<Self, SettingsError> {
        if let Some(url) = controller_url.as_deref() {
            validate_controller_url(url)?;
        }

        Ok(Self {
            bind_addr,
            controller_url,
            log_profile,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsError {
    InvalidBindAddr(String),
    InvalidLogProfile(String),
    UnsupportedUrlScheme(String),
}

impl Display for SettingsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBindAddr(value) => write!(formatter, "invalid bind address: {value}"),
            Self::InvalidLogProfile(value) => write!(formatter, "invalid log profile: {value}"),
            Self::UnsupportedUrlScheme(value) => write!(
                formatter,
                "controller URL must start with http:// or https://: {value}"
            ),
        }
    }
}

impl std::error::Error for SettingsError {}

pub fn parse_bind_addr(value: &str) -> Result<SocketAddr, SettingsError> {
    value
        .parse()
        .map_err(|_| SettingsError::InvalidBindAddr(value.to_owned()))
}

fn validate_controller_url(url: &str) -> Result<(), SettingsError> {
    if is_https_url(url) {
        return Ok(());
    }

    if is_http_url(url) {
        return Ok(());
    }

    Err(SettingsError::UnsupportedUrlScheme(url.to_owned()))
}

fn is_https_url(url: &str) -> bool {
    url.starts_with("https://")
}

fn is_http_url(url: &str) -> bool {
    url.starts_with("http://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_bind_addr() {
        let addr = parse_bind_addr("127.0.0.1:7700").expect("bind address should parse");
        assert_eq!(addr.to_string(), "127.0.0.1:7700");
    }

    #[test]
    fn rejects_invalid_bind_addr() {
        let error = parse_bind_addr("not-an-address").expect_err("bind address should fail");
        assert_eq!(
            error,
            SettingsError::InvalidBindAddr("not-an-address".to_owned())
        );
    }

    #[test]
    fn rejects_invalid_log_profile() {
        let error = "verbose"
            .parse::<LogProfile>()
            .expect_err("log profile should fail");
        assert_eq!(
            error,
            SettingsError::InvalidLogProfile("verbose".to_owned())
        );
    }

    #[test]
    fn accepts_http_controller_url() {
        let bind_addr = parse_bind_addr("127.0.0.1:7700").expect("valid bind address");
        let settings = Settings::new(
            bind_addr,
            Some("http://10.0.0.5:7700".to_owned()),
            LogProfile::Product,
        )
        .expect("http URL should be allowed with warnings at runtime boundaries");

        assert_eq!(
            settings.controller_url.as_deref(),
            Some("http://10.0.0.5:7700")
        );
    }

    #[test]
    fn rejects_unsupported_controller_url_scheme() {
        let bind_addr = parse_bind_addr("127.0.0.1:7700").expect("valid bind address");
        let error = Settings::new(
            bind_addr,
            Some("ftp://controller.example.com".to_owned()),
            LogProfile::Product,
        )
        .expect_err("unsupported scheme should fail");

        assert_eq!(
            error,
            SettingsError::UnsupportedUrlScheme("ftp://controller.example.com".to_owned())
        );
    }
}
