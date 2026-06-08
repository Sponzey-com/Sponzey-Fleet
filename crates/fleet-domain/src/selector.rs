use crate::agent::{Agent, AgentLabel};
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    AgentName(String),
    Labels(Vec<AgentLabel>),
}

impl Selector {
    pub fn parse(value: &str) -> Result<Self, SelectorError> {
        if let Some(agent) = value.strip_prefix("agent:") {
            if agent.trim().is_empty() {
                return Err(SelectorError::Invalid(value.to_owned()));
            }
            return Ok(Self::AgentName(agent.to_owned()));
        }

        let mut labels = Vec::new();
        for part in value.split(',') {
            let Some((key, label_value)) = part.split_once('=') else {
                return Err(SelectorError::Invalid(value.to_owned()));
            };
            labels.push(
                AgentLabel::new(key, label_value)
                    .map_err(|_| SelectorError::Invalid(value.to_owned()))?,
            );
        }
        if labels.is_empty() {
            Err(SelectorError::Invalid(value.to_owned()))
        } else {
            Ok(Self::Labels(labels))
        }
    }

    pub fn matches(&self, agent: &Agent) -> bool {
        match self {
            Self::AgentName(name) => agent.name().as_str() == name,
            Self::Labels(expected) => expected.iter().all(|label| {
                agent
                    .labels()
                    .iter()
                    .any(|actual| actual.key() == label.key() && actual.value() == label.value())
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectorError {
    Invalid(String),
}

impl Display for SelectorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(value) => write!(f, "invalid selector: {value}"),
        }
    }
}

impl std::error::Error for SelectorError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentFingerprint, AgentId, AgentIdentity, AgentName, AgentPublicKey};

    fn agent() -> Agent {
        let mut agent = Agent::new(
            AgentId::new("a1").unwrap(),
            AgentName::new("web-01").unwrap(),
            AgentIdentity {
                public_key: AgentPublicKey::new("pk").unwrap(),
                fingerprint: AgentFingerprint::new("0123456789abcdef").unwrap(),
            },
        );
        agent.set_labels(vec![
            AgentLabel::new("role", "web").unwrap(),
            AgentLabel::new("env", "prod").unwrap(),
        ]);
        agent
    }

    #[test]
    fn parses_label_selector() {
        assert!(matches!(
            Selector::parse("role=web").unwrap(),
            Selector::Labels(_)
        ));
    }

    #[test]
    fn rejects_invalid_selector() {
        assert!(Selector::parse("role:web").is_err());
    }

    #[test]
    fn matches_label_selector() {
        assert!(Selector::parse("role=web").unwrap().matches(&agent()));
    }

    #[test]
    fn rejects_label_mismatch() {
        assert!(!Selector::parse("role=db").unwrap().matches(&agent()));
    }

    #[test]
    fn matches_agent_name_selector() {
        assert!(Selector::parse("agent:web-01").unwrap().matches(&agent()));
    }
}
