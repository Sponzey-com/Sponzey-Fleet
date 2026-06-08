use fleet_domain::{
    AgentId, DriftReport, DriftStatus, JobError, PackageState, Policy, PolicyCheck, Runbook,
    RunbookTask, ServicePrimitiveState, ServiceState, TaskEnvelope,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fmt::{Display, Formatter};
use std::fs;
use std::io::Read;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub timeout: Duration,
    pub max_output_bytes: usize,
    pub env: BTreeMap<String, String>,
}

impl CommandSpec {
    pub fn new(program: impl Into<String>, args: Vec<String>, timeout: Duration) -> Self {
        Self {
            program: program.into(),
            args,
            timeout,
            max_output_bytes: 1024 * 1024,
            env: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerError {
    Io(String),
    Job(JobError),
    InvalidSignature,
    ReplayedNonce,
    Timeout,
    Canceled,
    OutputLimitExceeded,
    Stream(String),
    HighRiskConfirmationRequired(String),
    Primitive(String),
}

impl Display for RunnerError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "runner io error: {error}"),
            Self::Job(error) => write!(formatter, "{error}"),
            Self::InvalidSignature => write!(formatter, "task envelope signature is invalid"),
            Self::ReplayedNonce => write!(formatter, "task envelope nonce was already used"),
            Self::Timeout => write!(formatter, "command timed out"),
            Self::Canceled => write!(formatter, "command was canceled"),
            Self::OutputLimitExceeded => write!(formatter, "command output limit exceeded"),
            Self::Stream(error) => write!(formatter, "command stream error: {error}"),
            Self::HighRiskConfirmationRequired(step_id) => {
                write!(
                    formatter,
                    "high-risk runbook step requires confirmation: {step_id}"
                )
            }
            Self::Primitive(error) => write!(formatter, "primitive execution error: {error}"),
        }
    }
}

impl std::error::Error for RunnerError {}

impl From<std::io::Error> for RunnerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<PrimitiveError> for RunnerError {
    fn from(value: PrimitiveError) -> Self {
        Self::Primitive(value.to_string())
    }
}

pub trait TaskSignatureVerifier {
    fn verify(&self, payload_hash: &str, signature: &str) -> bool;
}

#[derive(Debug, Default)]
pub struct NonceReplayGuard {
    seen: HashSet<String>,
}

impl NonceReplayGuard {
    pub fn accept_once(&mut self, nonce: &str) -> Result<(), RunnerError> {
        if self.seen.insert(nonce.to_owned()) {
            Ok(())
        } else {
            Err(RunnerError::ReplayedNonce)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandOutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutputChunk {
    pub stream: CommandOutputStream,
    pub sequence: u64,
    pub data: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxPackageManager {
    Apt,
    Dnf,
    Yum,
    Apk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitiveCommand {
    pub program: String,
    pub args: Vec<String>,
    pub high_risk: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunbookExecutionPlan {
    pub runbook_name: String,
    pub steps: Vec<RunbookExecutionStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunbookExecutionOptions {
    pub confirmed_high_risk: bool,
    pub command_timeout: Duration,
    pub max_output_bytes: usize,
}

impl Default for RunbookExecutionOptions {
    fn default() -> Self {
        Self {
            confirmed_high_risk: false,
            command_timeout: Duration::from_secs(60),
            max_output_bytes: 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunbookExecutionReport {
    pub runbook_name: String,
    pub outcomes: Vec<RunbookStepOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunbookStepOutcome {
    pub id: String,
    pub changed: Option<bool>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub audit_metadata: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunbookExecutionStep {
    pub id: String,
    pub action: RunbookExecutionAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunbookExecutionAction {
    Command(PrimitiveCommand),
    FileCopy(FileCopySpec),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitiveCheckStatus {
    pub desired: String,
    pub actual: String,
    pub changed: Option<bool>,
}

impl PrimitiveCheckStatus {
    pub fn already_desired(desired: impl Into<String>, actual: impl Into<String>) -> Self {
        Self {
            desired: desired.into(),
            actual: actual.into(),
            changed: Some(false),
        }
    }

    pub fn would_change(desired: impl Into<String>, actual: impl Into<String>) -> Self {
        Self {
            desired: desired.into(),
            actual: actual.into(),
            changed: Some(true),
        }
    }

    pub fn unknown(desired: impl Into<String>, actual: impl Into<String>) -> Self {
        Self {
            desired: desired.into(),
            actual: actual.into(),
            changed: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemdActiveState {
    Active,
    Inactive,
    Failed,
    Activating,
    Deactivating,
    Unknown,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimitiveError {
    EmptyName,
    UnsupportedPackageManager,
    UnsafeServiceName(String),
    UnsafePath(String),
    ParentDirectoryMissing(String),
    PermissionDenied(String),
    Io(String),
}

impl Display for PrimitiveError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyName => write!(formatter, "primitive name cannot be empty"),
            Self::UnsupportedPackageManager => write!(formatter, "unsupported package manager"),
            Self::UnsafeServiceName(name) => write!(formatter, "unsafe service name: {name}"),
            Self::UnsafePath(path) => write!(formatter, "unsafe file destination: {path}"),
            Self::ParentDirectoryMissing(path) => {
                write!(
                    formatter,
                    "file destination parent directory is missing: {path}"
                )
            }
            Self::PermissionDenied(path) => write!(formatter, "permission denied: {path}"),
            Self::Io(error) => write!(formatter, "primitive io error: {error}"),
        }
    }
}

impl std::error::Error for PrimitiveError {}

impl From<std::io::Error> for PrimitiveError {
    fn from(value: std::io::Error) -> Self {
        primitive_error_from_io(None, value)
    }
}

pub fn detect_linux_package_manager(paths: &[&str]) -> Option<LinuxPackageManager> {
    if paths.iter().any(|path| path.ends_with("/apt-get")) {
        Some(LinuxPackageManager::Apt)
    } else if paths.iter().any(|path| path.ends_with("/dnf")) {
        Some(LinuxPackageManager::Dnf)
    } else if paths.iter().any(|path| path.ends_with("/yum")) {
        Some(LinuxPackageManager::Yum)
    } else if paths.iter().any(|path| path.ends_with("/apk")) {
        Some(LinuxPackageManager::Apk)
    } else {
        None
    }
}

pub fn detect_local_linux_package_manager() -> Option<LinuxPackageManager> {
    const CANDIDATES: &[(&str, LinuxPackageManager)] = &[
        ("/usr/bin/apt-get", LinuxPackageManager::Apt),
        ("/bin/apt-get", LinuxPackageManager::Apt),
        ("/usr/bin/dnf", LinuxPackageManager::Dnf),
        ("/bin/dnf", LinuxPackageManager::Dnf),
        ("/usr/bin/yum", LinuxPackageManager::Yum),
        ("/bin/yum", LinuxPackageManager::Yum),
        ("/sbin/apk", LinuxPackageManager::Apk),
        ("/usr/bin/apk", LinuxPackageManager::Apk),
    ];
    CANDIDATES
        .iter()
        .find_map(|(path, manager)| Path::new(path).exists().then_some(*manager))
}

pub fn package_install_command(
    manager: LinuxPackageManager,
    package_name: &str,
) -> Result<PrimitiveCommand, PrimitiveError> {
    validate_simple_name(package_name).map_err(|_| PrimitiveError::EmptyName)?;
    let (program, args) = match manager {
        LinuxPackageManager::Apt => (
            "apt-get",
            vec!["install", "-y", "--no-install-recommends", package_name],
        ),
        LinuxPackageManager::Dnf => ("dnf", vec!["install", "-y", package_name]),
        LinuxPackageManager::Yum => ("yum", vec!["install", "-y", package_name]),
        LinuxPackageManager::Apk => ("apk", vec!["add", package_name]),
    };
    Ok(PrimitiveCommand {
        program: program.to_owned(),
        args: args.into_iter().map(str::to_owned).collect(),
        high_risk: true,
    })
}

pub fn package_present_check_command(
    package_name: &str,
) -> Result<PrimitiveCommand, PrimitiveError> {
    validate_simple_name(package_name).map_err(|_| PrimitiveError::EmptyName)?;
    Ok(PrimitiveCommand {
        program: "sh".to_owned(),
        args: vec![
            "-c".to_owned(),
            format!(
                "dpkg -s {package_name} >/dev/null 2>&1 || rpm -q {package_name} >/dev/null 2>&1 || apk info -e {package_name} >/dev/null 2>&1"
            ),
        ],
        high_risk: false,
    })
}

pub fn package_present_status(
    package_name: &str,
    check_exit_code: i32,
) -> Result<PrimitiveCheckStatus, PrimitiveError> {
    validate_simple_name(package_name).map_err(|_| PrimitiveError::EmptyName)?;
    let desired = format!("package {package_name} present");
    if check_exit_code == 0 {
        Ok(PrimitiveCheckStatus::already_desired(desired, "present"))
    } else {
        Ok(PrimitiveCheckStatus::would_change(desired, "missing"))
    }
}

pub fn systemd_service_status_command(
    service_name: &str,
) -> Result<PrimitiveCommand, PrimitiveError> {
    validate_service_name(service_name)?;
    Ok(PrimitiveCommand {
        program: "systemctl".to_owned(),
        args: vec!["is-active".to_owned(), service_name.to_owned()],
        high_risk: false,
    })
}

pub fn parse_systemd_active_state(stdout: &str, exit_code: Option<i32>) -> SystemdActiveState {
    match stdout.trim() {
        "active" => SystemdActiveState::Active,
        "inactive" => SystemdActiveState::Inactive,
        "failed" => SystemdActiveState::Failed,
        "activating" => SystemdActiveState::Activating,
        "deactivating" => SystemdActiveState::Deactivating,
        "unknown" => SystemdActiveState::Unknown,
        _ if exit_code.is_none() => SystemdActiveState::Unavailable,
        _ => SystemdActiveState::Unknown,
    }
}

pub fn systemd_service_running_status(
    service_name: &str,
    stdout: &str,
    exit_code: Option<i32>,
) -> Result<PrimitiveCheckStatus, PrimitiveError> {
    validate_service_name(service_name)?;
    let desired = format!("service {service_name} running");
    let state = parse_systemd_active_state(stdout, exit_code);
    Ok(match state {
        SystemdActiveState::Active => PrimitiveCheckStatus::already_desired(desired, "active"),
        SystemdActiveState::Inactive
        | SystemdActiveState::Failed
        | SystemdActiveState::Activating
        | SystemdActiveState::Deactivating => {
            PrimitiveCheckStatus::would_change(desired, format!("{state:?}").to_lowercase())
        }
        SystemdActiveState::Unknown | SystemdActiveState::Unavailable => {
            PrimitiveCheckStatus::unknown(desired, format!("{state:?}").to_lowercase())
        }
    })
}

pub fn systemd_service_start_command(
    service_name: &str,
) -> Result<PrimitiveCommand, PrimitiveError> {
    validate_service_name(service_name)?;
    Ok(PrimitiveCommand {
        program: "systemctl".to_owned(),
        args: vec!["start".to_owned(), service_name.to_owned()],
        high_risk: true,
    })
}

pub fn systemd_service_restart_command(
    service_name: &str,
) -> Result<PrimitiveCommand, PrimitiveError> {
    validate_service_name(service_name)?;
    Ok(PrimitiveCommand {
        program: "systemctl".to_owned(),
        args: vec!["restart".to_owned(), service_name.to_owned()],
        high_risk: true,
    })
}

pub fn systemd_service_enable_command(
    service_name: &str,
) -> Result<PrimitiveCommand, PrimitiveError> {
    validate_service_name(service_name)?;
    Ok(PrimitiveCommand {
        program: "systemctl".to_owned(),
        args: vec!["enable".to_owned(), service_name.to_owned()],
        high_risk: true,
    })
}

pub fn build_runbook_execution_plan(
    runbook: &Runbook,
    package_manager: LinuxPackageManager,
) -> Result<RunbookExecutionPlan, PrimitiveError> {
    let mut steps = Vec::new();
    for task in &runbook.tasks {
        match task {
            RunbookTask::Package(package) => match package.state {
                PackageState::Present => {
                    steps.push(RunbookExecutionStep {
                        id: format!("{}:check", package.id),
                        action: RunbookExecutionAction::Command(package_present_check_command(
                            &package.name,
                        )?),
                    });
                    steps.push(RunbookExecutionStep {
                        id: format!("{}:install", package.id),
                        action: RunbookExecutionAction::Command(package_install_command(
                            package_manager,
                            &package.name,
                        )?),
                    });
                }
            },
            RunbookTask::Service(service) => {
                steps.push(RunbookExecutionStep {
                    id: format!("{}:status", service.id),
                    action: RunbookExecutionAction::Command(systemd_service_status_command(
                        &service.name,
                    )?),
                });
                if service.enabled == Some(true) {
                    steps.push(RunbookExecutionStep {
                        id: format!("{}:enable", service.id),
                        action: RunbookExecutionAction::Command(systemd_service_enable_command(
                            &service.name,
                        )?),
                    });
                }
                let command = match service.state {
                    ServicePrimitiveState::Started => systemd_service_start_command(&service.name)?,
                    ServicePrimitiveState::Restarted => {
                        systemd_service_restart_command(&service.name)?
                    }
                };
                steps.push(RunbookExecutionStep {
                    id: format!("{}:apply", service.id),
                    action: RunbookExecutionAction::Command(command),
                });
            }
            RunbookTask::FileCopy(copy) => {
                steps.push(RunbookExecutionStep {
                    id: format!("{}:copy", copy.id),
                    action: RunbookExecutionAction::FileCopy(FileCopySpec {
                        destination: PathBuf::from(&copy.dest),
                        content: copy.content.as_bytes().to_vec(),
                        mode: copy.mode.as_deref().and_then(parse_octal_mode),
                    }),
                });
            }
        }
    }
    Ok(RunbookExecutionPlan {
        runbook_name: runbook.name.clone(),
        steps,
    })
}

pub fn execute_runbook_execution_plan(
    plan: &RunbookExecutionPlan,
    options: RunbookExecutionOptions,
) -> Result<RunbookExecutionReport, RunnerError> {
    execute_runbook_execution_plan_with(
        plan,
        options,
        |command, spec| {
            let mut spec = spec.clone();
            spec.program = command.program.clone();
            spec.args = command.args.clone();
            run_command_with_spec(spec)
        },
        copy_file_atomic,
    )
}

pub fn execute_runbook_execution_plan_with(
    plan: &RunbookExecutionPlan,
    options: RunbookExecutionOptions,
    mut run_command: impl FnMut(&PrimitiveCommand, &CommandSpec) -> Result<CommandOutput, RunnerError>,
    mut copy_file: impl FnMut(&FileCopySpec) -> Result<FileCopyResult, PrimitiveError>,
) -> Result<RunbookExecutionReport, RunnerError> {
    let mut outcomes = Vec::new();
    for step in &plan.steps {
        let outcome = match &step.action {
            RunbookExecutionAction::Command(command) => {
                if command.high_risk && !options.confirmed_high_risk {
                    return Err(RunnerError::HighRiskConfirmationRequired(step.id.clone()));
                }
                let mut spec = CommandSpec::new(
                    command.program.clone(),
                    command.args.clone(),
                    options.command_timeout,
                );
                spec.max_output_bytes = options.max_output_bytes;
                let output = run_command(command, &spec)?;
                RunbookStepOutcome {
                    id: step.id.clone(),
                    changed: if command.high_risk { Some(true) } else { None },
                    exit_code: Some(output.exit_code),
                    stdout: output.stdout,
                    stderr: output.stderr,
                    audit_metadata: format!(
                        "primitive=command,program={},high_risk={}",
                        command.program, command.high_risk
                    ),
                }
            }
            RunbookExecutionAction::FileCopy(spec) => {
                let result = copy_file(spec)?;
                RunbookStepOutcome {
                    id: step.id.clone(),
                    changed: Some(result.changed),
                    exit_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    audit_metadata: result.audit_metadata,
                }
            }
        };
        outcomes.push(outcome);
    }
    Ok(RunbookExecutionReport {
        runbook_name: plan.runbook_name.clone(),
        outcomes,
    })
}

fn parse_octal_mode(value: &str) -> Option<u32> {
    u32::from_str_radix(value.trim_start_matches('0'), 8).ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCopySpec {
    pub destination: PathBuf,
    pub content: Vec<u8>,
    pub mode: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCopyResult {
    pub changed: bool,
    pub before_checksum: Option<String>,
    pub after_checksum: String,
    pub bytes_written: usize,
    pub audit_metadata: String,
    pub atomic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckObservation {
    Match { expected: String, actual: String },
    Mismatch { expected: String, actual: String },
    Unknown { expected: String, actual: String },
}

pub trait DriftProbe {
    fn service_running(&self, name: &str) -> CheckObservation;
    fn package_present(&self, name: &str) -> CheckObservation;
    fn file_sha256(&self, path: &str, expected_sha256: &str) -> CheckObservation;
}

pub struct LocalDriftProbe;

impl DriftProbe for LocalDriftProbe {
    fn service_running(&self, name: &str) -> CheckObservation {
        let expected = format!("service {name} running");
        let Ok(command) = systemd_service_status_command(name) else {
            return CheckObservation::Unknown {
                expected,
                actual: "invalid service name".to_owned(),
            };
        };
        match run_command_with_spec(CommandSpec::new(
            command.program,
            command.args,
            Duration::from_secs(5),
        )) {
            Ok(output) if output.exit_code == 0 && output.stdout.trim() == "active" => {
                CheckObservation::Match {
                    expected,
                    actual: "active".to_owned(),
                }
            }
            Ok(output) => CheckObservation::Mismatch {
                expected,
                actual: output.stdout.trim().to_owned(),
            },
            Err(error) => CheckObservation::Unknown {
                expected,
                actual: error.to_string(),
            },
        }
    }

    fn package_present(&self, name: &str) -> CheckObservation {
        let expected = format!("package {name} present");
        let Ok(command) = package_present_check_command(name) else {
            return CheckObservation::Unknown {
                expected,
                actual: "invalid package name".to_owned(),
            };
        };
        match run_command_with_spec(CommandSpec::new(
            command.program,
            command.args,
            Duration::from_secs(5),
        )) {
            Ok(output) if output.exit_code == 0 => CheckObservation::Match {
                expected,
                actual: "present".to_owned(),
            },
            Ok(_) => CheckObservation::Mismatch {
                expected,
                actual: "missing".to_owned(),
            },
            Err(error) => CheckObservation::Unknown {
                expected,
                actual: error.to_string(),
            },
        }
    }

    fn file_sha256(&self, path: &str, expected_sha256: &str) -> CheckObservation {
        let expected = format!("file {path} sha256 {expected_sha256}");
        match fs::read(path) {
            Ok(body) => {
                let actual_sha256 = sha256_hex(&body);
                if actual_sha256 == expected_sha256 {
                    CheckObservation::Match {
                        expected,
                        actual: actual_sha256,
                    }
                } else {
                    CheckObservation::Mismatch {
                        expected,
                        actual: actual_sha256,
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                CheckObservation::Mismatch {
                    expected,
                    actual: "missing".to_owned(),
                }
            }
            Err(error) => CheckObservation::Unknown {
                expected,
                actual: error.to_string(),
            },
        }
    }
}

pub fn evaluate_policy_drift(policy: &Policy, probe: &impl DriftProbe) -> DriftReport {
    let observations = policy
        .checks
        .iter()
        .map(|check| evaluate_policy_check(check, probe))
        .collect::<Vec<_>>();
    let expected = observations
        .iter()
        .map(CheckObservation::expected)
        .collect::<Vec<_>>()
        .join("; ");
    let actual = observations
        .iter()
        .map(CheckObservation::actual)
        .collect::<Vec<_>>()
        .join("; ");
    let status = if observations
        .iter()
        .any(|observation| matches!(observation, CheckObservation::Mismatch { .. }))
    {
        DriftStatus::Drifted
    } else if observations
        .iter()
        .any(|observation| matches!(observation, CheckObservation::Unknown { .. }))
    {
        DriftStatus::Unknown
    } else {
        DriftStatus::Compliant
    };

    DriftReport {
        policy_name: policy.name.clone(),
        status,
        expected,
        actual,
    }
}

fn evaluate_policy_check(check: &PolicyCheck, probe: &impl DriftProbe) -> CheckObservation {
    match check {
        PolicyCheck::Service {
            name,
            state: ServiceState::Running,
            ..
        } => probe.service_running(name),
        PolicyCheck::Package { name, present, .. } if *present => probe.package_present(name),
        PolicyCheck::Package { name, .. } => CheckObservation::Unknown {
            expected: format!("package {name} absent"),
            actual: "package absent checks are not supported in MVP".to_owned(),
        },
        PolicyCheck::FileChecksum { path, sha256, .. } => probe.file_sha256(path, sha256),
    }
}

impl CheckObservation {
    fn expected(&self) -> String {
        match self {
            Self::Match { expected, .. }
            | Self::Mismatch { expected, .. }
            | Self::Unknown { expected, .. } => expected.clone(),
        }
    }

    fn actual(&self) -> String {
        match self {
            Self::Match { actual, .. }
            | Self::Mismatch { actual, .. }
            | Self::Unknown { actual, .. } => actual.clone(),
        }
    }
}

pub fn copy_file_atomic(spec: &FileCopySpec) -> Result<FileCopyResult, PrimitiveError> {
    copy_file_with_rename(spec, |from, to| fs::rename(from, to))
}

fn copy_file_with_rename(
    spec: &FileCopySpec,
    mut rename: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<FileCopyResult, PrimitiveError> {
    validate_file_destination(&spec.destination)?;
    let parent = spec
        .destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| {
            PrimitiveError::ParentDirectoryMissing(spec.destination.display().to_string())
        })?;
    if !parent.is_dir() {
        return Err(PrimitiveError::ParentDirectoryMissing(
            parent.display().to_string(),
        ));
    }

    let before = fs::read(&spec.destination).ok();
    let before_checksum = before.as_ref().map(|body| sha256_hex(body));
    let after_checksum = sha256_hex(&spec.content);
    if before.as_deref() == Some(spec.content.as_slice()) {
        return Ok(FileCopyResult {
            changed: false,
            before_checksum,
            after_checksum,
            bytes_written: spec.content.len(),
            audit_metadata: format!(
                "destination={},changed=false,bytes={},atomic=true",
                spec.destination.display(),
                spec.content.len()
            ),
            atomic: true,
        });
    }

    let temp_path = temporary_copy_path(&spec.destination);
    fs::write(&temp_path, &spec.content)
        .map_err(|error| primitive_error_from_io(Some(&temp_path), error))?;
    if let Some(mode) = spec.mode {
        #[cfg(unix)]
        fs::set_permissions(&temp_path, fs::Permissions::from_mode(mode))
            .map_err(|error| primitive_error_from_io(Some(&temp_path), error))?;
    }
    let mut atomic = true;
    match rename(&temp_path, &spec.destination) {
        Ok(()) => {}
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                return Err(primitive_error_from_io(Some(&spec.destination), error));
            }
            atomic = false;
            fs::write(&spec.destination, &spec.content)
                .map_err(|error| primitive_error_from_io(Some(&spec.destination), error))?;
            if let Some(mode) = spec.mode {
                #[cfg(unix)]
                fs::set_permissions(&spec.destination, fs::Permissions::from_mode(mode))
                    .map_err(|error| primitive_error_from_io(Some(&spec.destination), error))?;
            }
        }
    }

    Ok(FileCopyResult {
        changed: true,
        before_checksum,
        after_checksum,
        bytes_written: spec.content.len(),
        audit_metadata: format!(
            "destination={},changed=true,bytes={},atomic={atomic}",
            spec.destination.display(),
            spec.content.len()
        ),
        atomic,
    })
}

fn primitive_error_from_io(path: Option<&Path>, error: std::io::Error) -> PrimitiveError {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        PrimitiveError::PermissionDenied(
            path.map(|path| path.display().to_string())
                .unwrap_or_else(|| error.to_string()),
        )
    } else {
        PrimitiveError::Io(error.to_string())
    }
}

fn validate_file_destination(path: &Path) -> Result<(), PrimitiveError> {
    if !path.is_absolute()
        || path == Path::new("/")
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(PrimitiveError::UnsafePath(path.display().to_string()));
    }
    Ok(())
}

fn temporary_copy_path(destination: &Path) -> PathBuf {
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("copy");
    destination.with_file_name(format!(".{file_name}.sponzey.tmp.{}", std::process::id()))
}

fn sha256_hex(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn validate_simple_name(value: &str) -> Result<(), ()> {
    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '+' | '-')
        })
    {
        Err(())
    } else {
        Ok(())
    }
}

fn validate_service_name(value: &str) -> Result<(), PrimitiveError> {
    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '@' | '-')
        })
    {
        Err(PrimitiveError::UnsafeServiceName(value.to_owned()))
    } else {
        Ok(())
    }
}

pub fn verify_envelope(
    envelope: &TaskEnvelope,
    agent_id: &AgentId,
    now: SystemTime,
) -> Result<(), JobError> {
    envelope.validate_for_agent(agent_id, now)
}

pub fn verify_signed_envelope(
    envelope: &TaskEnvelope,
    agent_id: &AgentId,
    now: SystemTime,
    verifier: &impl TaskSignatureVerifier,
) -> Result<(), RunnerError> {
    envelope
        .validate_for_agent(agent_id, now)
        .map_err(RunnerError::Job)?;
    let signature = envelope
        .signature
        .as_ref()
        .ok_or(RunnerError::Job(JobError::UnsignedTask))?;
    if verifier.verify(&envelope.payload_hash, signature.as_str()) {
        Ok(())
    } else {
        Err(RunnerError::InvalidSignature)
    }
}

pub fn verify_signed_envelope_once(
    envelope: &TaskEnvelope,
    agent_id: &AgentId,
    now: SystemTime,
    verifier: &impl TaskSignatureVerifier,
    replay_guard: &mut NonceReplayGuard,
) -> Result<(), RunnerError> {
    verify_signed_envelope(envelope, agent_id, now, verifier)?;
    replay_guard.accept_once(envelope.nonce.as_str())
}

pub fn run_command(program: &str, args: &[String]) -> std::io::Result<CommandOutput> {
    run_command_with_spec(CommandSpec::new(
        program,
        args.to_vec(),
        Duration::from_secs(60),
    ))
    .map_err(std::io::Error::other)
}

pub fn run_command_with_spec(spec: CommandSpec) -> Result<CommandOutput, RunnerError> {
    run_command_with_cancel(spec, || false)
}

pub fn run_command_with_cancel(
    spec: CommandSpec,
    mut should_cancel: impl FnMut() -> bool,
) -> Result<CommandOutput, RunnerError> {
    let started_at = Instant::now();
    let mut child = Command::new(&spec.program)
        .args(&spec.args)
        .envs(&spec.env)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    loop {
        if child.try_wait()?.is_some() {
            let output = child.wait_with_output()?;
            return command_output_from_process_output(output, spec.max_output_bytes);
        }

        if should_cancel() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RunnerError::Canceled);
        }

        if started_at.elapsed() >= spec.timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RunnerError::Timeout);
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn run_command_streaming(
    spec: CommandSpec,
    on_chunk: impl FnMut(CommandOutputChunk) -> Result<(), RunnerError>,
) -> Result<CommandOutput, RunnerError> {
    run_command_streaming_with_cancel(spec, || false, on_chunk)
}

pub fn run_command_streaming_with_cancel(
    spec: CommandSpec,
    mut should_cancel: impl FnMut() -> bool,
    mut on_chunk: impl FnMut(CommandOutputChunk) -> Result<(), RunnerError>,
) -> Result<CommandOutput, RunnerError> {
    let started_at = Instant::now();
    let mut child = Command::new(&spec.program)
        .args(&spec.args)
        .envs(&spec.env)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (sender, receiver) = mpsc::channel();
    let mut stdout_thread = stdout
        .map(|stream| spawn_output_reader(stream, CommandOutputStream::Stdout, sender.clone()));
    let mut stderr_thread = stderr
        .map(|stream| spawn_output_reader(stream, CommandOutputStream::Stderr, sender.clone()));
    drop(sender);

    let mut stdout_body = Vec::new();
    let mut stderr_body = Vec::new();
    let mut total_output = 0usize;
    let mut sequence = 0u64;

    loop {
        while let Ok(event) = receiver.try_recv() {
            let StreamingOutputEvent { stream, bytes } = event?;
            total_output += bytes.len();
            if total_output > spec.max_output_bytes {
                let _ = child.kill();
                let _ = child.wait();
                join_output_reader(&mut stdout_thread);
                join_output_reader(&mut stderr_thread);
                return Err(RunnerError::OutputLimitExceeded);
            }
            match stream {
                CommandOutputStream::Stdout => stdout_body.extend_from_slice(&bytes),
                CommandOutputStream::Stderr => stderr_body.extend_from_slice(&bytes),
            }
            on_chunk(CommandOutputChunk {
                stream,
                sequence,
                data: String::from_utf8_lossy(&bytes).to_string(),
            })?;
            sequence += 1;
        }

        if let Some(status) = child.try_wait()? {
            join_output_reader(&mut stdout_thread);
            join_output_reader(&mut stderr_thread);
            drain_streaming_receiver(
                &receiver,
                &mut stdout_body,
                &mut stderr_body,
                &mut total_output,
                spec.max_output_bytes,
                &mut sequence,
                &mut on_chunk,
            )?;
            return Ok(CommandOutput {
                stdout: String::from_utf8_lossy(&stdout_body).to_string(),
                stderr: String::from_utf8_lossy(&stderr_body).to_string(),
                exit_code: status.code().unwrap_or(-1),
                truncated: false,
            });
        }

        if should_cancel() {
            let _ = child.kill();
            let _ = child.wait();
            join_output_reader(&mut stdout_thread);
            join_output_reader(&mut stderr_thread);
            return Err(RunnerError::Canceled);
        }

        if started_at.elapsed() >= spec.timeout {
            let _ = child.kill();
            let _ = child.wait();
            join_output_reader(&mut stdout_thread);
            join_output_reader(&mut stderr_thread);
            return Err(RunnerError::Timeout);
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

struct StreamingOutputEvent {
    stream: CommandOutputStream,
    bytes: Vec<u8>,
}

fn spawn_output_reader<R>(
    mut reader: R,
    stream: CommandOutputStream,
    sender: mpsc::Sender<Result<StreamingOutputEvent, RunnerError>>,
) -> thread::JoinHandle<()>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = [0u8; 16 * 1024];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => return,
                Ok(count) => {
                    if sender
                        .send(Ok(StreamingOutputEvent {
                            stream,
                            bytes: buffer[..count].to_vec(),
                        }))
                        .is_err()
                    {
                        return;
                    }
                }
                Err(error) => {
                    let _ = sender.send(Err(RunnerError::Io(error.to_string())));
                    return;
                }
            }
        }
    })
}

fn drain_streaming_receiver(
    receiver: &mpsc::Receiver<Result<StreamingOutputEvent, RunnerError>>,
    stdout_body: &mut Vec<u8>,
    stderr_body: &mut Vec<u8>,
    total_output: &mut usize,
    max_output_bytes: usize,
    sequence: &mut u64,
    on_chunk: &mut impl FnMut(CommandOutputChunk) -> Result<(), RunnerError>,
) -> Result<(), RunnerError> {
    while let Ok(event) = receiver.try_recv() {
        let StreamingOutputEvent { stream, bytes } = event?;
        *total_output += bytes.len();
        if *total_output > max_output_bytes {
            return Err(RunnerError::OutputLimitExceeded);
        }
        match stream {
            CommandOutputStream::Stdout => stdout_body.extend_from_slice(&bytes),
            CommandOutputStream::Stderr => stderr_body.extend_from_slice(&bytes),
        }
        on_chunk(CommandOutputChunk {
            stream,
            sequence: *sequence,
            data: String::from_utf8_lossy(&bytes).to_string(),
        })?;
        *sequence += 1;
    }
    Ok(())
}

fn join_output_reader(handle: &mut Option<thread::JoinHandle<()>>) {
    if let Some(handle) = handle.take() {
        let _ = handle.join();
    }
}

fn command_output_from_process_output(
    output: std::process::Output,
    max_output_bytes: usize,
) -> Result<CommandOutput, RunnerError> {
    let total_output = output.stdout.len() + output.stderr.len();
    if total_output > max_output_bytes {
        return Err(RunnerError::OutputLimitExceeded);
    }
    Ok(CommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
        truncated: false,
    })
}

pub fn chunk_command_output(output: &CommandOutput, chunk_size: usize) -> Vec<CommandOutputChunk> {
    let mut chunks = Vec::new();
    push_chunks(
        &mut chunks,
        CommandOutputStream::Stdout,
        &output.stdout,
        chunk_size,
    );
    push_chunks(
        &mut chunks,
        CommandOutputStream::Stderr,
        &output.stderr,
        chunk_size,
    );
    chunks
}

fn push_chunks(
    chunks: &mut Vec<CommandOutputChunk>,
    stream: CommandOutputStream,
    data: &str,
    chunk_size: usize,
) {
    if data.is_empty() {
        return;
    }
    let chunk_size = chunk_size.max(1);
    for chunk in data.as_bytes().chunks(chunk_size) {
        chunks.push(CommandOutputChunk {
            stream,
            sequence: chunks.len() as u64,
            data: String::from_utf8_lossy(chunk).to_string(),
        });
    }
}

pub fn runner_layer_ready() -> bool {
    fleet_domain::DOMAIN_LAYER == "fleet-domain"
}

#[cfg(test)]
mod tests {
    use super::*;
    use fleet_domain::{
        JobId, Policy, PolicyCheck, Selector, ServiceState, TaskExpiry, TaskId, TaskNonce,
        TaskSignature,
    };
    use std::fs;
    use std::time::Duration;

    #[test]
    fn detects_linux_package_manager_from_paths() {
        assert_eq!(
            detect_linux_package_manager(&["/usr/bin/dnf", "/usr/bin/rpm"]),
            Some(LinuxPackageManager::Dnf)
        );
        assert_eq!(
            detect_linux_package_manager(&["/usr/bin/apt-get"]),
            Some(LinuxPackageManager::Apt)
        );
        assert_eq!(detect_linux_package_manager(&["/bin/unknown"]), None);
    }

    #[test]
    fn builds_package_install_command() {
        let command = package_install_command(LinuxPackageManager::Apt, "nginx").unwrap();

        assert_eq!(command.program, "apt-get");
        assert_eq!(
            command.args,
            ["install", "-y", "--no-install-recommends", "nginx"]
        );
        assert!(command.high_risk);
    }

    #[test]
    fn builds_package_present_check_command() {
        let command = package_present_check_command("nginx").unwrap();

        assert_eq!(command.program, "sh");
        assert!(command.args.join(" ").contains("dpkg -s nginx"));
        assert!(!command.high_risk);
    }

    #[test]
    fn package_already_installed_returns_changed_false() {
        let status = package_present_status("nginx", 0).unwrap();

        assert_eq!(status.actual, "present");
        assert_eq!(status.changed, Some(false));
    }

    #[test]
    fn package_missing_returns_changed_true() {
        let status = package_present_status("nginx", 1).unwrap();

        assert_eq!(status.actual, "missing");
        assert_eq!(status.changed, Some(true));
    }

    #[test]
    fn builds_systemd_service_commands_with_risk_boundary() {
        let status = systemd_service_status_command("nginx.service").unwrap();
        let start = systemd_service_start_command("nginx.service").unwrap();
        let restart = systemd_service_restart_command("nginx.service").unwrap();
        let enable = systemd_service_enable_command("nginx.service").unwrap();

        assert_eq!(status.args, ["is-active", "nginx.service"]);
        assert!(!status.high_risk);
        assert_eq!(start.args, ["start", "nginx.service"]);
        assert!(start.high_risk);
        assert_eq!(restart.args, ["restart", "nginx.service"]);
        assert!(restart.high_risk);
        assert_eq!(enable.args, ["enable", "nginx.service"]);
        assert!(enable.high_risk);
    }

    #[test]
    fn runbook_execution_plan_links_package_service_and_file_copy_primitives() {
        let runbook = fleet_domain::parse_runbook_document(
            r#"
apiVersion: fleet.sponzey.dev/v1alpha1
kind: Runbook
metadata:
  name: bootstrap-web
spec:
  targets:
    selector: role=web
  tasks:
    - id: nginx-package
      package:
        name: nginx
        state: present
    - id: nginx-config
      file.copy:
        dest: /etc/nginx/conf.d/sponzey.conf
        content: server { listen 8080; }
        mode: 0644
    - id: nginx-service
      service:
        name: nginx.service
        state: started
        enabled: true
"#,
        )
        .unwrap();

        let plan = build_runbook_execution_plan(&runbook, LinuxPackageManager::Apt).unwrap();

        assert_eq!(plan.runbook_name, "bootstrap-web");
        assert_eq!(plan.steps.len(), 6);
        assert_eq!(plan.steps[0].id, "nginx-package:check");
        assert_eq!(plan.steps[1].id, "nginx-package:install");
        assert!(matches!(
            &plan.steps[2].action,
            RunbookExecutionAction::FileCopy(spec)
                if spec.destination == Path::new("/etc/nginx/conf.d/sponzey.conf")
                    && spec.mode == Some(0o644)
        ));
        assert!(matches!(
            &plan.steps[3].action,
            RunbookExecutionAction::Command(command)
                if command.program == "systemctl"
                    && command.args == ["is-active", "nginx.service"]
                    && !command.high_risk
        ));
        assert!(matches!(
            &plan.steps[4].action,
            RunbookExecutionAction::Command(command)
                if command.program == "systemctl"
                    && command.args == ["enable", "nginx.service"]
                    && command.high_risk
        ));
        assert!(matches!(
            &plan.steps[5].action,
            RunbookExecutionAction::Command(command)
                if command.program == "systemctl"
                    && command.args == ["start", "nginx.service"]
                    && command.high_risk
        ));
    }

    #[test]
    fn runbook_execution_rejects_unconfirmed_high_risk_command() {
        let plan = RunbookExecutionPlan {
            runbook_name: "high-risk".to_owned(),
            steps: vec![RunbookExecutionStep {
                id: "install:package".to_owned(),
                action: RunbookExecutionAction::Command(PrimitiveCommand {
                    program: "apt-get".to_owned(),
                    args: vec!["install".to_owned(), "-y".to_owned(), "nginx".to_owned()],
                    high_risk: true,
                }),
            }],
        };

        let result = execute_runbook_execution_plan_with(
            &plan,
            RunbookExecutionOptions::default(),
            |_command, _spec| panic!("unconfirmed high-risk command must not execute"),
            |_spec| panic!("file copy must not execute"),
        );

        assert_eq!(
            result,
            Err(RunnerError::HighRiskConfirmationRequired(
                "install:package".to_owned()
            ))
        );
    }

    #[test]
    fn runbook_execution_runs_file_copy_step() {
        let dir = temp_test_dir("runbook-file-copy");
        let destination = dir.join("app.conf");
        let runbook = fleet_domain::parse_runbook_document(&format!(
            r#"
apiVersion: fleet.sponzey.dev/v1alpha1
kind: Runbook
metadata:
  name: copy-config
spec:
  targets:
    selector: role=web
  tasks:
    - id: config
      file.copy:
        dest: {}
        content: worker_processes 2;
        mode: 0600
"#,
            destination.display()
        ))
        .unwrap();
        let plan = build_runbook_execution_plan(&runbook, LinuxPackageManager::Apt).unwrap();

        let report =
            execute_runbook_execution_plan(&plan, RunbookExecutionOptions::default()).unwrap();

        assert_eq!(report.runbook_name, "copy-config");
        assert_eq!(report.outcomes.len(), 1);
        assert_eq!(report.outcomes[0].id, "config:copy");
        assert_eq!(report.outcomes[0].changed, Some(true));
        assert!(report.outcomes[0].audit_metadata.contains("changed=true"));
        assert_eq!(
            fs::read_to_string(&destination).unwrap(),
            "worker_processes 2;"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn runbook_execution_invokes_confirmed_command_steps() {
        let plan = RunbookExecutionPlan {
            runbook_name: "commands".to_owned(),
            steps: vec![
                RunbookExecutionStep {
                    id: "check".to_owned(),
                    action: RunbookExecutionAction::Command(PrimitiveCommand {
                        program: "sh".to_owned(),
                        args: vec!["-c".to_owned(), "true".to_owned()],
                        high_risk: false,
                    }),
                },
                RunbookExecutionStep {
                    id: "start".to_owned(),
                    action: RunbookExecutionAction::Command(PrimitiveCommand {
                        program: "systemctl".to_owned(),
                        args: vec!["start".to_owned(), "nginx.service".to_owned()],
                        high_risk: true,
                    }),
                },
            ],
        };
        let mut executed = Vec::new();

        let report = execute_runbook_execution_plan_with(
            &plan,
            RunbookExecutionOptions {
                confirmed_high_risk: true,
                ..RunbookExecutionOptions::default()
            },
            |command, spec| {
                executed.push((command.program.clone(), spec.args.clone()));
                Ok(CommandOutput {
                    stdout: "ok".to_owned(),
                    stderr: String::new(),
                    exit_code: 0,
                    truncated: false,
                })
            },
            |_spec| panic!("file copy must not execute"),
        )
        .unwrap();

        assert_eq!(
            executed,
            vec![
                ("sh".to_owned(), vec!["-c".to_owned(), "true".to_owned()]),
                (
                    "systemctl".to_owned(),
                    vec!["start".to_owned(), "nginx.service".to_owned()]
                )
            ]
        );
        assert_eq!(report.outcomes[0].changed, None);
        assert_eq!(report.outcomes[1].changed, Some(true));
    }

    #[test]
    #[ignore = "requires Linux, root privileges, a package manager, and systemd"]
    fn manual_linux_nginx_runbook_executes() {
        assert_eq!(std::env::consts::OS, "linux");

        let package_manager_paths = [
            "/usr/bin/apt-get",
            "/bin/apt-get",
            "/usr/bin/dnf",
            "/bin/dnf",
            "/usr/bin/yum",
            "/bin/yum",
            "/sbin/apk",
            "/usr/bin/apk",
        ];
        let package_manager = detect_linux_package_manager(&package_manager_paths)
            .expect("manual smoke requires apt-get, dnf, yum, or apk");
        assert!(
            Path::new("/bin/systemctl").exists() || Path::new("/usr/bin/systemctl").exists(),
            "manual smoke requires systemd"
        );

        let body = fs::read_to_string("examples/runbooks/nginx-basic.yml")
            .expect("nginx runbook fixture must be readable from workspace root");
        let runbook = fleet_domain::parse_runbook_document(&body).unwrap();
        let plan = build_runbook_execution_plan(&runbook, package_manager).unwrap();
        let report = execute_runbook_execution_plan(
            &plan,
            RunbookExecutionOptions {
                confirmed_high_risk: true,
                command_timeout: Duration::from_secs(180),
                ..RunbookExecutionOptions::default()
            },
        )
        .unwrap();

        assert_eq!(report.runbook_name, "nginx-basic");
        assert!(report.outcomes.iter().any(|outcome| {
            outcome.id == "nginx-service:apply" && outcome.exit_code == Some(0)
        }));
    }

    #[test]
    fn parses_systemd_service_status() {
        assert_eq!(
            parse_systemd_active_state("active\n", Some(0)),
            SystemdActiveState::Active
        );
        assert_eq!(
            parse_systemd_active_state("inactive\n", Some(3)),
            SystemdActiveState::Inactive
        );
        assert_eq!(
            parse_systemd_active_state("failed\n", Some(3)),
            SystemdActiveState::Failed
        );
    }

    #[test]
    fn systemd_unavailable_returns_unknown_changed_state() {
        let status = systemd_service_running_status("nginx.service", "", None).unwrap();

        assert_eq!(status.actual, "unavailable");
        assert_eq!(status.changed, None);
    }

    #[test]
    fn service_already_running_returns_changed_false() {
        let status = systemd_service_running_status("nginx.service", "active\n", Some(0)).unwrap();

        assert_eq!(status.actual, "active");
        assert_eq!(status.changed, Some(false));
    }

    #[test]
    fn stopped_service_returns_changed_true() {
        let status =
            systemd_service_running_status("nginx.service", "inactive\n", Some(3)).unwrap();

        assert_eq!(status.actual, "inactive");
        assert_eq!(status.changed, Some(true));
    }

    #[test]
    fn rejects_unsafe_service_name() {
        assert!(matches!(
            systemd_service_start_command("nginx;reboot"),
            Err(PrimitiveError::UnsafeServiceName(_))
        ));
    }

    #[test]
    fn copy_file_creates_file_with_checksum_metadata() {
        let dir = temp_test_dir("copy-create");
        let destination = dir.join("app.conf");

        let result = copy_file_atomic(&FileCopySpec {
            destination: destination.clone(),
            content: b"worker_processes 2;\n".to_vec(),
            mode: Some(0o600),
        })
        .unwrap();

        assert!(result.changed);
        assert_eq!(result.before_checksum, None);
        assert_eq!(result.bytes_written, 20);
        assert!(result.audit_metadata.contains("changed=true"));
        assert_eq!(
            fs::read_to_string(&destination).unwrap(),
            "worker_processes 2;\n"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn unchanged_file_copy_returns_changed_false() {
        let dir = temp_test_dir("copy-unchanged");
        let destination = dir.join("app.conf");
        fs::write(&destination, "same").unwrap();

        let result = copy_file_atomic(&FileCopySpec {
            destination: destination.clone(),
            content: b"same".to_vec(),
            mode: None,
        })
        .unwrap();

        assert!(!result.changed);
        assert_eq!(result.before_checksum, Some(result.after_checksum.clone()));
        assert_eq!(fs::read_to_string(&destination).unwrap(), "same");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn file_copy_rejects_unsafe_destination() {
        assert!(matches!(
            copy_file_atomic(&FileCopySpec {
                destination: PathBuf::from("../relative.conf"),
                content: Vec::new(),
                mode: None,
            }),
            Err(PrimitiveError::UnsafePath(_))
        ));
    }

    #[test]
    fn file_copy_maps_missing_parent_directory() {
        let dir = temp_test_dir("copy-missing-parent");
        let destination = dir.join("missing").join("app.conf");

        assert!(matches!(
            copy_file_atomic(&FileCopySpec {
                destination,
                content: b"body".to_vec(),
                mode: None,
            }),
            Err(PrimitiveError::ParentDirectoryMissing(_))
        ));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn file_copy_maps_permission_denied() {
        let dir = temp_test_dir("copy-permission-denied");
        let destination = dir.join("app.conf");

        let result = copy_file_with_rename(
            &FileCopySpec {
                destination: destination.clone(),
                content: b"body".to_vec(),
                mode: None,
            },
            |_from, _to| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "denied",
                ))
            },
        );

        assert!(
            matches!(result, Err(PrimitiveError::PermissionDenied(path)) if path == destination.display().to_string())
        );
        assert!(!destination.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn file_copy_falls_back_when_atomic_rename_is_unavailable() {
        let dir = temp_test_dir("copy-fallback");
        let destination = dir.join("app.conf");

        let result = copy_file_with_rename(
            &FileCopySpec {
                destination: destination.clone(),
                content: b"fallback".to_vec(),
                mode: None,
            },
            |_from, _to| Err(std::io::Error::other("cross-device link")),
        )
        .unwrap();

        assert!(result.changed);
        assert!(!result.atomic);
        assert!(result.audit_metadata.contains("atomic=false"));
        assert_eq!(fs::read_to_string(&destination).unwrap(), "fallback");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn drift_engine_reports_compliant_when_all_checks_match() {
        let probe = StaticDriftProbe {
            service_running: CheckObservation::Match {
                expected: "service nginx running".to_owned(),
                actual: "active".to_owned(),
            },
            package_present: CheckObservation::Match {
                expected: "package nginx present".to_owned(),
                actual: "present".to_owned(),
            },
            file_sha256: CheckObservation::Match {
                expected: "file /etc/nginx/nginx.conf sha256 abc".to_owned(),
                actual: "abc".to_owned(),
            },
        };

        let report = evaluate_policy_drift(&policy_fixture(), &probe);

        assert_eq!(report.status, DriftStatus::Compliant);
        assert!(report.expected.contains("service nginx running"));
        assert!(report.actual.contains("present"));
    }

    #[test]
    fn drift_engine_reports_drifted_for_stopped_service() {
        let probe = StaticDriftProbe {
            service_running: CheckObservation::Mismatch {
                expected: "service nginx running".to_owned(),
                actual: "inactive".to_owned(),
            },
            package_present: CheckObservation::Match {
                expected: "package nginx present".to_owned(),
                actual: "present".to_owned(),
            },
            file_sha256: CheckObservation::Match {
                expected: "file /etc/nginx/nginx.conf sha256 abc".to_owned(),
                actual: "abc".to_owned(),
            },
        };

        let report = evaluate_policy_drift(&policy_fixture(), &probe);

        assert_eq!(report.status, DriftStatus::Drifted);
        assert!(report.actual.contains("inactive"));
    }

    #[test]
    fn drift_engine_reports_drifted_for_missing_package() {
        let probe = StaticDriftProbe {
            service_running: CheckObservation::Match {
                expected: "service nginx running".to_owned(),
                actual: "active".to_owned(),
            },
            package_present: CheckObservation::Mismatch {
                expected: "package nginx present".to_owned(),
                actual: "missing".to_owned(),
            },
            file_sha256: CheckObservation::Match {
                expected: "file /etc/nginx/nginx.conf sha256 abc".to_owned(),
                actual: "abc".to_owned(),
            },
        };

        let report = evaluate_policy_drift(&policy_fixture(), &probe);

        assert_eq!(report.status, DriftStatus::Drifted);
        assert!(report.actual.contains("missing"));
    }

    #[test]
    fn drift_engine_reports_unknown_separately_from_drifted() {
        let probe = StaticDriftProbe {
            service_running: CheckObservation::Unknown {
                expected: "service nginx running".to_owned(),
                actual: "systemd unavailable".to_owned(),
            },
            package_present: CheckObservation::Match {
                expected: "package nginx present".to_owned(),
                actual: "present".to_owned(),
            },
            file_sha256: CheckObservation::Match {
                expected: "file /etc/nginx/nginx.conf sha256 abc".to_owned(),
                actual: "abc".to_owned(),
            },
        };

        let report = evaluate_policy_drift(&policy_fixture(), &probe);

        assert_eq!(report.status, DriftStatus::Unknown);
        assert!(report.actual.contains("systemd unavailable"));
    }

    #[test]
    fn local_file_checksum_probe_detects_mismatch() {
        let dir = temp_test_dir("drift-file-checksum");
        let path = dir.join("config.txt");
        fs::write(&path, "actual").unwrap();

        let observation = LocalDriftProbe.file_sha256(path.to_str().unwrap(), "expected");

        assert!(matches!(observation, CheckObservation::Mismatch { .. }));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_unsigned_envelope() {
        let envelope = TaskEnvelope {
            job_id: JobId::new("j1").unwrap(),
            task_id: TaskId::new("t1").unwrap(),
            target_agent_id: AgentId::new("a1").unwrap(),
            issued_at: SystemTime::UNIX_EPOCH,
            expires_at: TaskExpiry::new(SystemTime::UNIX_EPOCH + Duration::from_secs(60)),
            nonce: TaskNonce::new("n1").unwrap(),
            payload_hash: "h".into(),
            signature: None,
        };
        assert_eq!(
            verify_envelope(
                &envelope,
                &AgentId::new("a1").unwrap(),
                SystemTime::UNIX_EPOCH
            ),
            Err(JobError::UnsignedTask)
        );
    }

    #[test]
    fn accepts_signed_envelope() {
        let envelope = TaskEnvelope {
            job_id: JobId::new("j1").unwrap(),
            task_id: TaskId::new("t1").unwrap(),
            target_agent_id: AgentId::new("a1").unwrap(),
            issued_at: SystemTime::UNIX_EPOCH,
            expires_at: TaskExpiry::new(SystemTime::UNIX_EPOCH + Duration::from_secs(60)),
            nonce: TaskNonce::new("n1").unwrap(),
            payload_hash: "h".into(),
            signature: Some(TaskSignature::new("sig").unwrap()),
        };
        assert!(
            verify_envelope(
                &envelope,
                &AgentId::new("a1").unwrap(),
                SystemTime::UNIX_EPOCH
            )
            .is_ok()
        );
    }

    #[test]
    fn invalid_signature_is_rejected() {
        let verifier = StaticVerifier { valid: false };
        assert_eq!(
            verify_signed_envelope(
                &signed_envelope("n1", SystemTime::UNIX_EPOCH + Duration::from_secs(60)),
                &AgentId::new("a1").unwrap(),
                SystemTime::UNIX_EPOCH,
                &verifier
            ),
            Err(RunnerError::InvalidSignature)
        );
    }

    #[test]
    fn expired_task_is_rejected() {
        let verifier = StaticVerifier { valid: true };
        assert_eq!(
            verify_signed_envelope(
                &signed_envelope("n1", SystemTime::UNIX_EPOCH),
                &AgentId::new("a1").unwrap(),
                SystemTime::UNIX_EPOCH,
                &verifier
            ),
            Err(RunnerError::Job(JobError::ExpiredTask))
        );
    }

    #[test]
    fn target_mismatch_is_rejected() {
        let verifier = StaticVerifier { valid: true };
        assert_eq!(
            verify_signed_envelope(
                &signed_envelope("n1", SystemTime::UNIX_EPOCH + Duration::from_secs(60)),
                &AgentId::new("a2").unwrap(),
                SystemTime::UNIX_EPOCH,
                &verifier
            ),
            Err(RunnerError::Job(JobError::TargetAgentMismatch))
        );
    }

    #[test]
    fn replayed_nonce_is_rejected() {
        let verifier = StaticVerifier { valid: true };
        let mut replay_guard = NonceReplayGuard::default();
        let envelope = signed_envelope("n1", SystemTime::UNIX_EPOCH + Duration::from_secs(60));

        verify_signed_envelope_once(
            &envelope,
            &AgentId::new("a1").unwrap(),
            SystemTime::UNIX_EPOCH,
            &verifier,
            &mut replay_guard,
        )
        .unwrap();

        assert_eq!(
            verify_signed_envelope_once(
                &envelope,
                &AgentId::new("a1").unwrap(),
                SystemTime::UNIX_EPOCH,
                &verifier,
                &mut replay_guard,
            ),
            Err(RunnerError::ReplayedNonce)
        );
    }

    #[test]
    fn successful_command_returns_stdout_and_exit_code() {
        let output =
            run_command_with_spec(shell_spec("printf ok", Duration::from_secs(5))).unwrap();

        assert_eq!(output.stdout, "ok");
        assert_eq!(output.stderr, "");
        assert_eq!(output.exit_code, 0);
    }

    #[test]
    fn streaming_command_emits_chunks_before_returning_output() {
        let mut chunks = Vec::new();

        let output = run_command_streaming(
            shell_spec("printf out; printf err >&2", Duration::from_secs(5)),
            |chunk| {
                chunks.push(chunk);
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(output.stdout, "out");
        assert_eq!(output.stderr, "err");
        assert_eq!(output.exit_code, 0);
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.stream == CommandOutputStream::Stdout && chunk.data == "out")
        );
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.stream == CommandOutputStream::Stderr && chunk.data == "err")
        );
    }

    #[test]
    fn non_zero_exit_code_is_preserved() {
        let output = run_command_with_spec(shell_spec("exit 7", Duration::from_secs(5))).unwrap();

        assert_eq!(output.exit_code, 7);
    }

    #[test]
    fn command_timeout_is_enforced() {
        let result = run_command_with_spec(shell_spec("sleep 1", Duration::from_millis(10)));

        assert_eq!(result, Err(RunnerError::Timeout));
    }

    #[test]
    fn command_cancel_is_enforced() {
        let mut polls = 0;
        let result = run_command_with_cancel(shell_spec("sleep 1", Duration::from_secs(5)), || {
            polls += 1;
            polls >= 1
        });

        assert_eq!(result, Err(RunnerError::Canceled));
    }

    #[test]
    fn output_limit_is_enforced() {
        let mut spec = shell_spec("printf abc", Duration::from_secs(5));
        spec.max_output_bytes = 2;

        assert_eq!(
            run_command_with_spec(spec),
            Err(RunnerError::OutputLimitExceeded)
        );
    }

    #[test]
    fn per_command_env_is_passed_without_global_mutation() {
        let mut spec = shell_spec("printf %s \"$SPONZEY_TEST_VALUE\"", Duration::from_secs(5));
        spec.env
            .insert("SPONZEY_TEST_VALUE".to_owned(), "from-command".to_owned());

        let output = run_command_with_spec(spec).unwrap();

        assert_eq!(output.stdout, "from-command");
    }

    #[test]
    fn output_chunks_keep_sequence_order() {
        let output = CommandOutput {
            stdout: "abcd".to_owned(),
            stderr: "ef".to_owned(),
            exit_code: 0,
            truncated: false,
        };

        let chunks = chunk_command_output(&output, 2);

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].sequence, 0);
        assert_eq!(chunks[0].stream, CommandOutputStream::Stdout);
        assert_eq!(chunks[0].data, "ab");
        assert_eq!(chunks[1].sequence, 1);
        assert_eq!(chunks[1].data, "cd");
        assert_eq!(chunks[2].sequence, 2);
        assert_eq!(chunks[2].stream, CommandOutputStream::Stderr);
        assert_eq!(chunks[2].data, "ef");
    }

    fn signed_envelope(nonce: &str, expires_at: SystemTime) -> TaskEnvelope {
        TaskEnvelope {
            job_id: JobId::new("j1").unwrap(),
            task_id: TaskId::new("t1").unwrap(),
            target_agent_id: AgentId::new("a1").unwrap(),
            issued_at: SystemTime::UNIX_EPOCH,
            expires_at: TaskExpiry::new(expires_at),
            nonce: TaskNonce::new(nonce).unwrap(),
            payload_hash: "h".into(),
            signature: Some(TaskSignature::new("sig").unwrap()),
        }
    }

    struct StaticVerifier {
        valid: bool,
    }

    impl TaskSignatureVerifier for StaticVerifier {
        fn verify(&self, _payload_hash: &str, _signature: &str) -> bool {
            self.valid
        }
    }

    #[cfg(unix)]
    fn shell_spec(script: &str, timeout: Duration) -> CommandSpec {
        CommandSpec::new("sh", vec!["-c".to_owned(), script.to_owned()], timeout)
    }

    #[cfg(windows)]
    fn shell_spec(script: &str, timeout: Duration) -> CommandSpec {
        CommandSpec::new("cmd", vec!["/C".to_owned(), script.to_owned()], timeout)
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "sponzey-fleet-runner-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn policy_fixture() -> Policy {
        Policy {
            name: "nginx-running".to_owned(),
            selector: Selector::parse("role=web").unwrap(),
            checks: vec![
                PolicyCheck::Service {
                    id: "service-nginx".to_owned(),
                    name: "nginx.service".to_owned(),
                    state: ServiceState::Running,
                },
                PolicyCheck::Package {
                    id: "package-nginx".to_owned(),
                    name: "nginx".to_owned(),
                    present: true,
                },
                PolicyCheck::FileChecksum {
                    id: "file-nginx".to_owned(),
                    path: "/etc/nginx/nginx.conf".to_owned(),
                    sha256: "abc".to_owned(),
                },
            ],
        }
    }

    struct StaticDriftProbe {
        service_running: CheckObservation,
        package_present: CheckObservation,
        file_sha256: CheckObservation,
    }

    impl DriftProbe for StaticDriftProbe {
        fn service_running(&self, _name: &str) -> CheckObservation {
            self.service_running.clone()
        }

        fn package_present(&self, _name: &str) -> CheckObservation {
            self.package_present.clone()
        }

        fn file_sha256(&self, _path: &str, _expected_sha256: &str) -> CheckObservation {
            self.file_sha256.clone()
        }
    }
}
