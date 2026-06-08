use crate::{Selector, SelectorError};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    pub name: String,
    pub selector: Selector,
    pub checks: Vec<PolicyCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyCheck {
    Service {
        id: String,
        name: String,
        state: ServiceState,
    },
    Package {
        id: String,
        name: String,
        present: bool,
    },
    FileChecksum {
        id: String,
        path: String,
        sha256: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceState {
    Running,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriftStatus {
    Compliant,
    Drifted,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftReport {
    pub policy_name: String,
    pub status: DriftStatus,
    pub expected: String,
    pub actual: String,
}

impl DriftReport {
    pub fn drifted(
        policy_name: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self {
            policy_name: policy_name.into(),
            status: DriftStatus::Drifted,
            expected: expected.into(),
            actual: actual.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyParseError {
    MissingField(&'static str),
    UnsupportedKind(String),
    UnsupportedCheck(String),
    UnsupportedServiceState(String),
    InvalidSelector(String),
    RemediationRequiresApproval,
}

impl Display for PolicyParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField(field) => {
                write!(formatter, "policy missing required field: {field}")
            }
            Self::UnsupportedKind(kind) => write!(formatter, "unsupported policy kind: {kind}"),
            Self::UnsupportedCheck(check) => write!(formatter, "unsupported policy check: {check}"),
            Self::UnsupportedServiceState(state) => {
                write!(formatter, "unsupported service state: {state}")
            }
            Self::InvalidSelector(selector) => {
                write!(formatter, "invalid policy selector: {selector}")
            }
            Self::RemediationRequiresApproval => {
                write!(
                    formatter,
                    "policy remediation requires approvalRequired: true in MVP"
                )
            }
        }
    }
}

impl std::error::Error for PolicyParseError {}

impl From<SelectorError> for PolicyParseError {
    fn from(value: SelectorError) -> Self {
        Self::InvalidSelector(value.to_string())
    }
}

pub fn parse_policy_document(body: &str) -> Result<Policy, PolicyParseError> {
    let mut api_version = None;
    let mut kind = None;
    let mut name = None;
    let mut selector_labels = BTreeMap::new();
    let mut checks = Vec::new();
    let mut current_check: Option<CheckBuilder> = None;
    let mut in_match_labels = false;
    let mut in_checks = false;
    let mut in_remediation = false;
    let mut remediation_approval_required = false;

    for raw_line in body.lines() {
        let without_comment = raw_line.split('#').next().unwrap_or_default();
        if without_comment.trim().is_empty() {
            continue;
        }
        let indent = without_comment
            .chars()
            .take_while(|character| *character == ' ')
            .count();
        let line = without_comment.trim();

        if indent == 0 {
            in_match_labels = false;
            in_checks = line == "checks:" || line == "spec:";
            in_remediation = line == "remediation:";
        }

        if let Some(value) = scalar_value(line, "apiVersion") {
            api_version = Some(value.to_owned());
            continue;
        }
        if let Some(value) = scalar_value(line, "kind") {
            kind = Some(value.to_owned());
            continue;
        }
        if indent >= 2 && line == "matchLabels:" {
            in_match_labels = true;
            continue;
        }
        if indent >= 2 && line == "checks:" {
            in_checks = true;
            in_match_labels = false;
            continue;
        }
        if indent >= 2 && line == "remediation:" {
            in_remediation = true;
            in_match_labels = false;
            in_checks = false;
            continue;
        }
        if indent >= 2 && matches!(line, "approvalRequired: true") {
            remediation_approval_required = true;
            continue;
        }
        if indent >= 2 && matches!(line, "approvalRequired: false") {
            remediation_approval_required = false;
            continue;
        }
        if indent >= 2
            && name.is_none()
            && let Some(value) = scalar_value(line, "name")
        {
            name = Some(value.to_owned());
            continue;
        }

        if in_match_labels
            && indent >= 6
            && let Some((key, value)) = line.split_once(':')
        {
            selector_labels.insert(key.trim().to_owned(), value.trim().to_owned());
            continue;
        }

        if in_checks && indent >= 4 {
            if let Some(value) = line.strip_prefix("- id:") {
                if let Some(builder) = current_check.take() {
                    checks.push(builder.build()?);
                }
                current_check = Some(CheckBuilder::new(value.trim()));
                continue;
            }
            if let Some(builder) = current_check.as_mut() {
                match line {
                    "service:" => builder.kind = Some("service".to_owned()),
                    "package:" => builder.kind = Some("package".to_owned()),
                    "file:" => builder.kind = Some("file".to_owned()),
                    value if value.ends_with(':') => {
                        builder.kind = Some(value.trim_end_matches(':').to_owned());
                    }
                    _ => {
                        if let Some((key, value)) = line.split_once(':') {
                            builder
                                .fields
                                .insert(key.trim().to_owned(), value.trim().to_owned());
                        }
                    }
                }
            }
        }
    }

    if let Some(builder) = current_check.take() {
        checks.push(builder.build()?);
    }
    if in_remediation && !remediation_approval_required {
        return Err(PolicyParseError::RemediationRequiresApproval);
    }

    let _api_version = api_version.ok_or(PolicyParseError::MissingField("apiVersion"))?;
    let kind = kind.ok_or(PolicyParseError::MissingField("kind"))?;
    if kind != "Policy" {
        return Err(PolicyParseError::UnsupportedKind(kind));
    }
    let name = name.ok_or(PolicyParseError::MissingField("metadata.name"))?;
    if selector_labels.is_empty() {
        return Err(PolicyParseError::MissingField("spec.selector.matchLabels"));
    }
    if checks.is_empty() {
        return Err(PolicyParseError::MissingField("spec.checks"));
    }
    let selector = Selector::parse(
        &selector_labels
            .into_iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(","),
    )?;

    Ok(Policy {
        name,
        selector,
        checks,
    })
}

fn scalar_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.strip_prefix(key)?
        .strip_prefix(':')
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

struct CheckBuilder {
    id: String,
    kind: Option<String>,
    fields: BTreeMap<String, String>,
}

impl CheckBuilder {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            kind: None,
            fields: BTreeMap::new(),
        }
    }

    fn build(self) -> Result<PolicyCheck, PolicyParseError> {
        match self.kind.as_deref() {
            Some("service") => {
                let name = self
                    .fields
                    .get("name")
                    .ok_or(PolicyParseError::MissingField("service.name"))?
                    .to_owned();
                let state = self
                    .fields
                    .get("state")
                    .ok_or(PolicyParseError::MissingField("service.state"))?;
                if state != "running" {
                    return Err(PolicyParseError::UnsupportedServiceState(state.to_owned()));
                }
                Ok(PolicyCheck::Service {
                    id: self.id,
                    name,
                    state: ServiceState::Running,
                })
            }
            Some("package") => {
                let name = self
                    .fields
                    .get("name")
                    .ok_or(PolicyParseError::MissingField("package.name"))?
                    .to_owned();
                let present = self
                    .fields
                    .get("state")
                    .is_none_or(|value| value == "present");
                Ok(PolicyCheck::Package {
                    id: self.id,
                    name,
                    present,
                })
            }
            Some("file") => {
                let path = self
                    .fields
                    .get("path")
                    .ok_or(PolicyParseError::MissingField("file.path"))?
                    .to_owned();
                let sha256 = self
                    .fields
                    .get("sha256")
                    .ok_or(PolicyParseError::MissingField("file.sha256"))?
                    .to_owned();
                Ok(PolicyCheck::FileChecksum {
                    id: self.id,
                    path,
                    sha256,
                })
            }
            Some(kind) => Err(PolicyParseError::UnsupportedCheck(kind.to_owned())),
            None => Err(PolicyParseError::MissingField("check kind")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NGINX_RUNNING: &str = r#"
apiVersion: fleet.sponzey.dev/v1alpha1
kind: Policy
metadata:
  name: nginx-running
spec:
  selector:
    matchLabels:
      role: web
  checks:
    - id: nginx-service
      service:
        name: nginx
        state: running
"#;

    #[test]
    fn parses_valid_service_policy() {
        let policy = parse_policy_document(NGINX_RUNNING).unwrap();

        assert_eq!(policy.name, "nginx-running");
        assert!(matches!(policy.selector, Selector::Labels(_)));
        assert!(matches!(
            policy.checks[0],
            PolicyCheck::Service {
                ref name,
                state: ServiceState::Running,
                ..
            } if name == "nginx"
        ));
    }

    #[test]
    fn rejects_unsupported_check() {
        let body = NGINX_RUNNING.replace("service:", "shell:");

        assert!(matches!(
            parse_policy_document(&body),
            Err(PolicyParseError::UnsupportedCheck(_))
        ));
    }

    #[test]
    fn rejects_invalid_selector() {
        let body = NGINX_RUNNING.replace("role: web", "bad key: web");

        assert!(matches!(
            parse_policy_document(&body),
            Err(PolicyParseError::InvalidSelector(_))
        ));
    }

    #[test]
    fn rejects_remediation_without_approval() {
        let body = format!("{NGINX_RUNNING}\nremediation:\n  run: restart nginx\n");

        assert!(matches!(
            parse_policy_document(&body),
            Err(PolicyParseError::RemediationRequiresApproval)
        ));
    }

    #[test]
    fn parses_file_checksum_check() {
        let body = r#"
apiVersion: fleet.sponzey.dev/v1alpha1
kind: Policy
metadata:
  name: file-check
spec:
  selector:
    matchLabels:
      role: web
  checks:
    - id: config-file
      file:
        path: /etc/nginx/nginx.conf
        sha256: abc123
"#;

        let policy = parse_policy_document(body).unwrap();

        assert!(matches!(
            policy.checks[0],
            PolicyCheck::FileChecksum { ref path, .. } if path == "/etc/nginx/nginx.conf"
        ));
    }
}
