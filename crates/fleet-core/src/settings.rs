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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransportSecurityMode {
    #[default]
    TlsRequired,
    DevInsecureLoopbackOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub bind_addr: SocketAddr,
    pub controller_url: Option<String>,
    pub log_profile: LogProfile,
    pub transport_security_mode: TransportSecurityMode,
}

impl Settings {
    pub fn new(
        bind_addr: SocketAddr,
        controller_url: Option<String>,
        log_profile: LogProfile,
        transport_security_mode: TransportSecurityMode,
    ) -> Result<Self, SettingsError> {
        if let Some(url) = controller_url.as_deref() {
            validate_controller_url(url, transport_security_mode)?;
        }

        Ok(Self {
            bind_addr,
            controller_url,
            log_profile,
            transport_security_mode,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsError {
    InvalidBindAddr(String),
    InvalidLogProfile(String),
    InsecureRemoteUrl(String),
    UnsupportedUrlScheme(String),
}

impl Display for SettingsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBindAddr(value) => write!(formatter, "invalid bind address: {value}"),
            Self::InvalidLogProfile(value) => write!(formatter, "invalid log profile: {value}"),
            Self::InsecureRemoteUrl(value) => write!(
                formatter,
                "insecure transport is only allowed for loopback demo URLs: {value}"
            ),
            Self::UnsupportedUrlScheme(value) => write!(
                formatter,
                "controller URL must use https, except explicit loopback demo mode: {value}"
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

fn validate_controller_url(url: &str, mode: TransportSecurityMode) -> Result<(), SettingsError> {
    if is_https_url(url) {
        return Ok(());
    }

    if is_http_url(url) && mode == TransportSecurityMode::DevInsecureLoopbackOnly {
        if is_loopback_http_url(url) {
            return Ok(());
        }

        return Err(SettingsError::InsecureRemoteUrl(url.to_owned()));
    }

    Err(SettingsError::UnsupportedUrlScheme(url.to_owned()))
}

fn is_https_url(url: &str) -> bool {
    url.starts_with("https://")
}

fn is_http_url(url: &str) -> bool {
    url.starts_with("http://")
}

fn is_loopback_http_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let host_port_path = rest.split('/').next().unwrap_or_default();
    let host = host_port_path
        .strip_prefix('[')
        .and_then(|value| value.split(']').next())
        .unwrap_or_else(|| host_port_path.split(':').next().unwrap_or_default());

    matches!(host, "localhost" | "127.0.0.1" | "::1")
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
    fn rejects_insecure_remote_url() {
        let bind_addr = parse_bind_addr("127.0.0.1:7700").expect("valid bind address");
        let error = Settings::new(
            bind_addr,
            Some("http://10.0.0.5:7700".to_owned()),
            LogProfile::Product,
            TransportSecurityMode::DevInsecureLoopbackOnly,
        )
        .expect_err("remote insecure URL should fail");

        assert_eq!(
            error,
            SettingsError::InsecureRemoteUrl("http://10.0.0.5:7700".to_owned())
        );
    }

    #[test]
    fn accepts_insecure_loopback_url() {
        let bind_addr = parse_bind_addr("127.0.0.1:7700").expect("valid bind address");
        let settings = Settings::new(
            bind_addr,
            Some("http://127.0.0.1:7700".to_owned()),
            LogProfile::Product,
            TransportSecurityMode::DevInsecureLoopbackOnly,
        )
        .expect("loopback insecure URL should be allowed");

        assert_eq!(
            settings.transport_security_mode,
            TransportSecurityMode::DevInsecureLoopbackOnly
        );
    }
}
