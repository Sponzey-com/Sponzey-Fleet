use crate::{Selector, SelectorError};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runbook {
    pub name: String,
    pub target_selector: Selector,
    pub tasks: Vec<RunbookTask>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunbookTask {
    Package(PackagePrimitive),
    Service(ServicePrimitive),
    FileCopy(FileCopyPrimitive),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePrimitive {
    pub id: String,
    pub name: String,
    pub state: PackageState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageState {
    Present,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServicePrimitive {
    pub id: String,
    pub name: String,
    pub state: ServicePrimitiveState,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServicePrimitiveState {
    Started,
    Restarted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCopyPrimitive {
    pub id: String,
    pub dest: String,
    pub content: String,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunbookParseError {
    MissingField(&'static str),
    UnsupportedKind(String),
    UnknownTopLevelField(String),
    UnsupportedTask(String),
    UnsupportedPackageState(String),
    UnsupportedServiceState(String),
    InvalidSelector(String),
    InvalidYaml(String),
    UnsafeFileDestination(String),
}

impl Display for RunbookParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField(field) => {
                write!(formatter, "runbook missing required field: {field}")
            }
            Self::UnsupportedKind(kind) => write!(formatter, "unsupported runbook kind: {kind}"),
            Self::UnknownTopLevelField(field) => {
                write!(formatter, "unknown runbook top-level field: {field}")
            }
            Self::UnsupportedTask(task) => write!(formatter, "unsupported runbook task: {task}"),
            Self::UnsupportedPackageState(state) => {
                write!(formatter, "unsupported package state: {state}")
            }
            Self::UnsupportedServiceState(state) => {
                write!(formatter, "unsupported service state: {state}")
            }
            Self::InvalidSelector(selector) => {
                write!(formatter, "invalid runbook selector: {selector}")
            }
            Self::InvalidYaml(message) => write!(formatter, "invalid runbook yaml: {message}"),
            Self::UnsafeFileDestination(path) => {
                write!(formatter, "unsafe file destination: {path}")
            }
        }
    }
}

impl std::error::Error for RunbookParseError {}

impl From<SelectorError> for RunbookParseError {
    fn from(value: SelectorError) -> Self {
        Self::InvalidSelector(value.to_string())
    }
}

pub fn runbook_schema_json() -> &'static str {
    r#"{
  "type": "object",
  "required": ["apiVersion", "kind", "metadata", "spec"],
  "properties": {
    "apiVersion": {"const": "fleet.sponzey.dev/v1alpha1"},
    "kind": {"const": "Runbook"},
    "metadata": {
      "type": "object",
      "required": ["name"],
      "properties": {"name": {"type": "string"}}
    },
    "spec": {
      "type": "object",
      "required": ["targets", "tasks"],
      "properties": {
        "targets": {
          "type": "object",
          "required": ["selector"],
          "properties": {"selector": {"type": "string"}}
        },
        "tasks": {
          "type": "array",
          "items": {
            "oneOf": [
              {"required": ["id", "package"]},
              {"required": ["id", "service"]},
              {"required": ["id", "file.copy"]}
            ]
          }
        }
      }
    }
  },
  "additionalProperties": false
}"#
}

pub fn parse_runbook_document(body: &str) -> Result<Runbook, RunbookParseError> {
    let mut api_version = None;
    let mut kind = None;
    let mut name = None;
    let mut target_selector = None;
    let mut tasks = Vec::new();
    let mut current_task: Option<TaskBuilder> = None;

    for raw_line in body.lines() {
        let without_comment = raw_line.split('#').next().unwrap_or_default();
        if without_comment.trim().is_empty() {
            continue;
        }
        if without_comment.contains('\t') {
            return Err(RunbookParseError::InvalidYaml(
                "tabs are not supported in MVP runbooks".to_owned(),
            ));
        }
        let indent = without_comment
            .chars()
            .take_while(|character| *character == ' ')
            .count();
        let line = without_comment.trim();

        if indent == 0 {
            let key = line
                .split_once(':')
                .map(|(key, _)| key)
                .unwrap_or(line)
                .trim();
            if !matches!(key, "apiVersion" | "kind" | "metadata" | "spec") {
                return Err(RunbookParseError::UnknownTopLevelField(key.to_owned()));
            }
        }

        if let Some(value) = scalar_value(line, "apiVersion") {
            api_version = Some(value.to_owned());
            continue;
        }
        if let Some(value) = scalar_value(line, "kind") {
            kind = Some(value.to_owned());
            continue;
        }
        if indent >= 2
            && name.is_none()
            && let Some(value) = scalar_value(line, "name")
        {
            name = Some(value.to_owned());
            continue;
        }
        if indent >= 4
            && let Some(value) = scalar_value(line, "selector")
        {
            target_selector = Some(Selector::parse(value)?);
            continue;
        }

        if indent >= 4
            && let Some(value) = line.strip_prefix("- id:")
        {
            if let Some(builder) = current_task.take() {
                tasks.push(builder.build()?);
            }
            current_task = Some(TaskBuilder::new(value.trim()));
            continue;
        }

        if indent >= 6
            && let Some(builder) = current_task.as_mut()
        {
            match line {
                "package:" => builder.kind = Some("package".to_owned()),
                "service:" => builder.kind = Some("service".to_owned()),
                "file.copy:" => builder.kind = Some("file.copy".to_owned()),
                value if value.ends_with(':') => {
                    builder.kind = Some(value.trim_end_matches(':').to_owned());
                }
                _ => {
                    let Some((key, value)) = line.split_once(':') else {
                        return Err(RunbookParseError::InvalidYaml(line.to_owned()));
                    };
                    builder
                        .fields
                        .insert(key.trim().to_owned(), value.trim().to_owned());
                }
            }
        }
    }

    if let Some(builder) = current_task.take() {
        tasks.push(builder.build()?);
    }

    let _api_version = api_version.ok_or(RunbookParseError::MissingField("apiVersion"))?;
    let kind = kind.ok_or(RunbookParseError::MissingField("kind"))?;
    if kind != "Runbook" {
        return Err(RunbookParseError::UnsupportedKind(kind));
    }
    let name = name.ok_or(RunbookParseError::MissingField("metadata.name"))?;
    let target_selector =
        target_selector.ok_or(RunbookParseError::MissingField("spec.targets.selector"))?;
    if tasks.is_empty() {
        return Err(RunbookParseError::MissingField("spec.tasks"));
    }

    Ok(Runbook {
        name,
        target_selector,
        tasks,
    })
}

fn scalar_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.strip_prefix(key)?
        .strip_prefix(':')
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

struct TaskBuilder {
    id: String,
    kind: Option<String>,
    fields: BTreeMap<String, String>,
}

impl TaskBuilder {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            kind: None,
            fields: BTreeMap::new(),
        }
    }

    fn build(self) -> Result<RunbookTask, RunbookParseError> {
        match self.kind.as_deref() {
            Some("package") => {
                let name = required_field(&self.fields, "name", "package.name")?;
                let state = required_field(&self.fields, "state", "package.state")?;
                if state != "present" {
                    return Err(RunbookParseError::UnsupportedPackageState(state));
                }
                Ok(RunbookTask::Package(PackagePrimitive {
                    id: self.id,
                    name,
                    state: PackageState::Present,
                }))
            }
            Some("service") => {
                let name = required_field(&self.fields, "name", "service.name")?;
                let state = required_field(&self.fields, "state", "service.state")?;
                let state = match state.as_str() {
                    "started" => ServicePrimitiveState::Started,
                    "restarted" => ServicePrimitiveState::Restarted,
                    _ => return Err(RunbookParseError::UnsupportedServiceState(state)),
                };
                Ok(RunbookTask::Service(ServicePrimitive {
                    id: self.id,
                    name,
                    state,
                    enabled: self.fields.get("enabled").map(|value| value == "true"),
                }))
            }
            Some("file.copy") => {
                let dest = required_field(&self.fields, "dest", "file.copy.dest")?;
                validate_file_destination(&dest)?;
                Ok(RunbookTask::FileCopy(FileCopyPrimitive {
                    id: self.id,
                    dest,
                    content: required_field(&self.fields, "content", "file.copy.content")?,
                    mode: self.fields.get("mode").cloned(),
                }))
            }
            Some(kind) => Err(RunbookParseError::UnsupportedTask(kind.to_owned())),
            None => Err(RunbookParseError::MissingField("task kind")),
        }
    }
}

fn required_field(
    fields: &BTreeMap<String, String>,
    key: &str,
    field: &'static str,
) -> Result<String, RunbookParseError> {
    fields
        .get(key)
        .cloned()
        .filter(|value| !value.is_empty())
        .ok_or(RunbookParseError::MissingField(field))
}

fn validate_file_destination(path: &str) -> Result<(), RunbookParseError> {
    if !path.starts_with('/') || path == "/" || path.contains("/../") || path.ends_with("/..") {
        Err(RunbookParseError::UnsafeFileDestination(path.to_owned()))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NGINX_RUNBOOK: &str = r#"
apiVersion: fleet.sponzey.dev/v1alpha1
kind: Runbook
metadata:
  name: nginx-basic
spec:
  targets:
    selector: role=web
  tasks:
    - id: nginx-package
      package:
        name: nginx
        state: present
    - id: nginx-service
      service:
        name: nginx
        state: started
        enabled: true
"#;

    #[test]
    fn parses_valid_nginx_runbook() {
        let runbook = parse_runbook_document(NGINX_RUNBOOK).unwrap();

        assert_eq!(runbook.name, "nginx-basic");
        assert!(matches!(runbook.target_selector, Selector::Labels(_)));
        assert_eq!(runbook.tasks.len(), 2);
    }

    #[test]
    fn rejects_missing_targets() {
        let body = NGINX_RUNBOOK.replace("    selector: role=web", "");

        assert!(matches!(
            parse_runbook_document(&body),
            Err(RunbookParseError::MissingField("spec.targets.selector"))
        ));
    }

    #[test]
    fn rejects_missing_tasks() {
        let body = NGINX_RUNBOOK
            .lines()
            .take_while(|line| !line.trim().starts_with("tasks:"))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(matches!(
            parse_runbook_document(&body),
            Err(RunbookParseError::MissingField("spec.tasks"))
        ));
    }

    #[test]
    fn rejects_unsupported_task() {
        let body = NGINX_RUNBOOK.replace("package:", "shell:");

        assert!(matches!(
            parse_runbook_document(&body),
            Err(RunbookParseError::UnsupportedTask(_))
        ));
    }

    #[test]
    fn rejects_invalid_yaml_tabs() {
        let body = NGINX_RUNBOOK.replace("  name: nginx-basic", "\tname: nginx-basic");

        assert!(matches!(
            parse_runbook_document(&body),
            Err(RunbookParseError::InvalidYaml(_))
        ));
    }

    #[test]
    fn rejects_unknown_top_level_field() {
        let body = format!("{NGINX_RUNBOOK}\nvars:\n  answer: 42\n");

        assert!(matches!(
            parse_runbook_document(&body),
            Err(RunbookParseError::UnknownTopLevelField(_))
        ));
    }

    #[test]
    fn parses_file_copy_task_and_rejects_unsafe_destination() {
        let body = r#"
apiVersion: fleet.sponzey.dev/v1alpha1
kind: Runbook
metadata:
  name: file-copy
spec:
  targets:
    selector: role=web
  tasks:
    - id: copy-index
      file.copy:
        dest: /tmp/index.html
        content: hello
        mode: "0644"
"#;
        let runbook = parse_runbook_document(body).unwrap();
        assert!(matches!(runbook.tasks[0], RunbookTask::FileCopy(_)));

        let unsafe_body = body.replace("/tmp/index.html", "../index.html");
        assert!(matches!(
            parse_runbook_document(&unsafe_body),
            Err(RunbookParseError::UnsafeFileDestination(_))
        ));
    }

    #[test]
    fn schema_export_mentions_required_fields() {
        let schema = runbook_schema_json();
        assert!(schema.contains("\"apiVersion\""));
        assert!(schema.contains("\"targets\""));
        assert!(schema.contains("\"tasks\""));
        assert!(!schema.contains("ansible"));
    }
}
