use std::fmt::{Display, Formatter};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(value: impl Into<String>) -> Result<Self, AgentError> {
        non_empty(value.into(), "agent id").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentName(String);

impl AgentName {
    pub fn new(value: impl Into<String>) -> Result<Self, AgentError> {
        let value = non_empty(value.into(), "agent name")?;
        if value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            Ok(Self(value))
        } else {
            Err(AgentError::InvalidName(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentFingerprint(String);

impl AgentFingerprint {
    pub fn new(value: impl Into<String>) -> Result<Self, AgentError> {
        let value = non_empty(value.into(), "agent fingerprint")?;
        if value.len() >= 16 && value.chars().all(|c| c.is_ascii_hexdigit() || c == ':') {
            Ok(Self(value))
        } else {
            Err(AgentError::InvalidFingerprint(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentLabel {
    key: String,
    value: String,
}

impl AgentLabel {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Result<Self, AgentError> {
        let key = non_empty(key.into(), "label key")?;
        let value = non_empty(value.into(), "label value")?;
        if is_label_part(&key) && is_label_part(&value) {
            Ok(Self { key, value })
        } else {
            Err(AgentError::InvalidLabel(format!("{key}={value}")))
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPublicKey(String);

impl AgentPublicKey {
    pub fn new(value: impl Into<String>) -> Result<Self, AgentError> {
        non_empty(value.into(), "agent public key").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerPublicKey(String);

impl ControllerPublicKey {
    pub fn new(value: impl Into<String>) -> Result<Self, AgentError> {
        non_empty(value.into(), "controller public key").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentIdentity {
    pub public_key: AgentPublicKey,
    pub fingerprint: AgentFingerprint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Pending,
    Online,
    Busy,
    Degraded,
    Offline,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    id: AgentId,
    name: AgentName,
    identity: AgentIdentity,
    labels: Vec<AgentLabel>,
    status: AgentStatus,
    version: Option<String>,
    os: Option<String>,
    arch: Option<String>,
    capabilities: Vec<String>,
    last_seen_at: Option<SystemTime>,
    pinned_controller: Option<ControllerPublicKey>,
}

impl Agent {
    pub fn new(id: AgentId, name: AgentName, identity: AgentIdentity) -> Self {
        Self {
            id,
            name,
            identity,
            labels: Vec::new(),
            status: AgentStatus::Pending,
            version: None,
            os: None,
            arch: None,
            capabilities: Vec::new(),
            last_seen_at: None,
            pinned_controller: None,
        }
    }

    pub fn restore(
        id: AgentId,
        name: AgentName,
        identity: AgentIdentity,
        labels: Vec<AgentLabel>,
        status: AgentStatus,
        last_seen_at: Option<SystemTime>,
        pinned_controller: Option<ControllerPublicKey>,
    ) -> Self {
        Self {
            id,
            name,
            identity,
            labels,
            status,
            version: None,
            os: None,
            arch: None,
            capabilities: Vec::new(),
            last_seen_at,
            pinned_controller,
        }
    }

    pub fn id(&self) -> &AgentId {
        &self.id
    }

    pub fn name(&self) -> &AgentName {
        &self.name
    }

    pub fn status(&self) -> AgentStatus {
        self.status
    }

    pub fn identity(&self) -> &AgentIdentity {
        &self.identity
    }

    pub fn labels(&self) -> &[AgentLabel] {
        &self.labels
    }

    pub fn last_seen_at(&self) -> Option<SystemTime> {
        self.last_seen_at
    }

    pub fn pinned_controller(&self) -> Option<&ControllerPublicKey> {
        self.pinned_controller.as_ref()
    }

    pub fn set_labels(&mut self, labels: Vec<AgentLabel>) {
        self.labels = labels;
    }

    pub fn pin_controller(&mut self, key: ControllerPublicKey) {
        self.pinned_controller = Some(key);
    }

    pub fn mark_online(&mut self, at: SystemTime) -> Result<(), AgentError> {
        if self.status == AgentStatus::Disabled {
            return Err(AgentError::Disabled);
        }
        self.status = AgentStatus::Online;
        self.last_seen_at = Some(at);
        Ok(())
    }

    pub fn mark_offline(&mut self) {
        if self.status != AgentStatus::Disabled {
            self.status = AgentStatus::Offline;
        }
    }

    pub fn disable(&mut self) {
        self.status = AgentStatus::Disabled;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentError {
    Empty(&'static str),
    InvalidName(String),
    InvalidFingerprint(String),
    InvalidLabel(String),
    Disabled,
}

impl Display for AgentError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty(field) => write!(f, "{field} cannot be empty"),
            Self::InvalidName(value) => write!(f, "invalid agent name: {value}"),
            Self::InvalidFingerprint(value) => write!(f, "invalid agent fingerprint: {value}"),
            Self::InvalidLabel(value) => write!(f, "invalid agent label: {value}"),
            Self::Disabled => write!(f, "disabled agent cannot become online"),
        }
    }
}

impl std::error::Error for AgentError {}

fn non_empty(value: String, field: &'static str) -> Result<String, AgentError> {
    if value.trim().is_empty() {
        Err(AgentError::Empty(field))
    } else {
        Ok(value)
    }
}

fn is_label_part(value: &str) -> bool {
    value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> AgentIdentity {
        AgentIdentity {
            public_key: AgentPublicKey::new("agent-public-key").unwrap(),
            fingerprint: AgentFingerprint::new("0123456789abcdef").unwrap(),
        }
    }

    #[test]
    fn accepts_valid_label() {
        let label = AgentLabel::new("role", "web").unwrap();
        assert_eq!(label.key(), "role");
        assert_eq!(label.value(), "web");
    }

    #[test]
    fn rejects_invalid_label() {
        assert!(AgentLabel::new("role!", "web").is_err());
    }

    #[test]
    fn accepts_valid_agent_identity() {
        let identity = identity();
        assert_eq!(identity.fingerprint.as_str(), "0123456789abcdef");
    }

    #[test]
    fn rejects_invalid_fingerprint() {
        assert!(AgentFingerprint::new("short").is_err());
    }

    #[test]
    fn moves_pending_agent_online() {
        let mut agent = Agent::new(
            AgentId::new("a1").unwrap(),
            AgentName::new("web-01").unwrap(),
            identity(),
        );
        agent.mark_online(SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(agent.status(), AgentStatus::Online);
    }

    #[test]
    fn moves_online_agent_offline() {
        let mut agent = Agent::new(
            AgentId::new("a1").unwrap(),
            AgentName::new("web-01").unwrap(),
            identity(),
        );
        agent.mark_online(SystemTime::UNIX_EPOCH).unwrap();
        agent.mark_offline();
        assert_eq!(agent.status(), AgentStatus::Offline);
    }

    #[test]
    fn disabled_agent_cannot_become_online_without_enable() {
        let mut agent = Agent::new(
            AgentId::new("a1").unwrap(),
            AgentName::new("web-01").unwrap(),
            identity(),
        );
        agent.disable();
        assert_eq!(
            agent.mark_online(SystemTime::UNIX_EPOCH),
            Err(AgentError::Disabled)
        );
    }
}
