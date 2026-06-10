use axum::{
    Router,
    body::Bytes,
    extract::{
        State,
        ws::{Message as AxumWsMessage, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response as AxumResponse},
    routing::get,
};
use fleet_application::{
    AdminTokenRepository, AgentInventoryRepository, CommandJobRepository, CreateCommandJob,
    CreateCommandJobError, CreateCommandJobInput, CreateDriftCheckJob, CreateDriftCheckJobError,
    CreateDriftCheckJobInput, CreateEnrollmentToken, CreateEnrollmentTokenInput, CreateRunbookJob,
    CreateRunbookJobError, CreateRunbookJobInput, DriftRepository, EnrollmentTokenRepository,
    EnrollmentTokenUseCaseError, EnsureAdminToken, FactsRepository, GetInventoryAgent,
    GetLatestDrift, GetLatestFacts, GetLatestMetrics, JobOutputChunk, JobOutputRepository,
    JobOutputStream, JobQueryRepository, JobRepository, ListAuditEvents, ListEnrollmentTokens,
    ListInventoryAgents, ListJobOutputForJob, ListJobSummaries, MetricsRepository,
    RevokeEnrollmentToken, RevokeEnrollmentTokenInput, RunbookJobRepository,
    TaskAssignmentRepository, TaskEnvelopeSigner, UpdateAgentLabels, UpdateAgentLabelsError,
    UpdateAgentLabelsInput, VerifyAdminToken, select_dispatch_targets,
};
use fleet_domain::{
    Agent, AgentFingerprint, AgentId, AgentIdentity, AgentLabel, AgentName, AgentPublicKey,
    AgentStatus, AuditActor, AuditCategory, AuditEvent, AuditTarget, AuditValue,
    ControllerPublicKey, DriftReport, DriftStatus, Job, JobStatus, Selector, TaskEnvelope,
};
use fleet_store::SqliteStore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const ADMIN_INDEX_HTML: &str = include_str!("../../../web-admin/index.html");
const ADMIN_STYLES_CSS: &str = include_str!("../../../web-admin/styles.css");
const ADMIN_APP_JS: &str = include_str!("../../../web-admin/app.js");
const ADMIN_API_CLIENT_JS: &str = include_str!("../../../web-admin/api-client.js");
const ADMIN_API_SCHEMA_JSON: &str = include_str!("../../../web-admin/api.schema.json");

#[derive(Debug, Clone)]
pub struct ControllerServerConfig {
    pub host: String,
    pub port: u16,
    pub data_dir: PathBuf,
    pub database_path: Option<PathBuf>,
    pub dev_insecure_loopback: bool,
}

#[derive(Clone)]
struct ControllerAppState {
    store: Arc<Mutex<SqliteStore>>,
    identity: Arc<ControllerIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerIdentity {
    pub public_key: String,
    pub fingerprint: String,
    private_key: String,
}

impl ControllerIdentity {
    #[cfg(test)]
    fn dev_insecure() -> Self {
        Self {
            public_key: "dev-controller-public-key".to_owned(),
            fingerprint: "dev-controller-fingerprint".to_owned(),
            private_key: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollAgentRequest {
    pub token: String,
    pub agent_id: String,
    pub name: String,
    pub public_key: String,
    pub fingerprint: String,
    pub labels: Vec<EnrollAgentLabel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollAgentLabel {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollAgentResponse {
    pub agent_id: String,
    pub controller_public_key: String,
    pub controller_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerIdentityResponse {
    pub controller_public_key: String,
    pub controller_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEnrollmentTokenResponse {
    pub id: String,
    pub token: String,
    pub expires_in_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollmentTokenSummaryResponse {
    pub id: String,
    pub default_labels: String,
    pub max_uses: u32,
    pub used_count: u32,
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCommandJobRequest {
    pub job_id: String,
    pub target_agent_ids: Vec<String>,
    #[serde(default)]
    pub selector: Option<String>,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub timeout_seconds: u64,
    pub confirmed_high_risk: bool,
    #[serde(default = "default_confirmed_by")]
    pub confirmed_by: String,
    #[serde(default = "default_job_expiration_seconds")]
    pub expires_in_seconds: u64,
    #[serde(default)]
    pub nonce_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCommandJobResponse {
    pub job_id: String,
    pub target_count: usize,
    pub assignment_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDriftCheckJobRequest {
    pub job_id: String,
    pub target_agent_ids: Vec<String>,
    #[serde(default)]
    pub selector: Option<String>,
    pub policy_document: String,
    #[serde(default = "default_drift_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default = "default_confirmed_by")]
    pub created_by: String,
    #[serde(default = "default_job_expiration_seconds")]
    pub expires_in_seconds: u64,
    #[serde(default)]
    pub nonce_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDriftCheckJobResponse {
    pub job_id: String,
    pub target_count: usize,
    pub assignment_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRunbookJobRequest {
    pub job_id: String,
    pub target_agent_ids: Vec<String>,
    #[serde(default)]
    pub selector: Option<String>,
    pub runbook_document: String,
    #[serde(default = "default_drift_timeout_seconds")]
    pub timeout_seconds: u64,
    pub confirmed_high_risk: bool,
    #[serde(default = "default_confirmed_by")]
    pub confirmed_by: String,
    #[serde(default = "default_job_expiration_seconds")]
    pub expires_in_seconds: u64,
    #[serde(default)]
    pub nonce_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRunbookJobResponse {
    pub job_id: String,
    pub target_count: usize,
    pub assignment_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSummaryResponse {
    pub id: String,
    pub status: String,
    pub risk: String,
    pub command_program: Option<String>,
    pub command_args: Vec<String>,
    pub target_count: usize,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobOutputChunkResponse {
    pub job_id: String,
    pub agent_id: String,
    pub stream: String,
    pub sequence: u64,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLabelResponse {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub id: String,
    pub name: String,
    pub status: String,
    pub fingerprint: String,
    pub labels: Vec<AgentLabelResponse>,
    pub last_seen_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAgentLabelsRequest {
    pub labels: Vec<AgentLabelResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestFactsResponse {
    pub agent_id: String,
    pub collected_at_ms: u64,
    pub body: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestMetricsResponse {
    pub agent_id: String,
    pub collected_at_ms: u64,
    pub body: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestDriftReportResponse {
    pub agent_id: String,
    pub checked_at_ms: u64,
    pub policy_name: String,
    pub status: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEventResponse {
    pub category: String,
    pub action: String,
    pub actor: String,
    pub target: String,
    pub value_kind: String,
    pub value: String,
    pub occurred_at_ms: u64,
}

#[derive(Debug)]
pub enum ControllerError {
    Io(std::io::Error),
    Store(fleet_store::StoreError),
    Protocol(fleet_protocol::ProtocolError),
    Json(String),
    InsecureRemote(String),
}

impl Display for ControllerError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Store(error) => write!(formatter, "store error: {error:?}"),
            Self::Protocol(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "json error: {error}"),
            Self::InsecureRemote(host) => {
                write!(
                    formatter,
                    "dev-insecure-loopback is only allowed for loopback hosts: {host}"
                )
            }
        }
    }
}

impl std::error::Error for ControllerError {}

impl From<std::io::Error> for ControllerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<fleet_store::StoreError> for ControllerError {
    fn from(value: fleet_store::StoreError) -> Self {
        Self::Store(value)
    }
}

impl From<fleet_protocol::ProtocolError> for ControllerError {
    fn from(value: fleet_protocol::ProtocolError) -> Self {
        Self::Protocol(value)
    }
}

pub fn start_controller_server(config: ControllerServerConfig) -> Result<(), ControllerError> {
    start_controller_server_until(config, || false)
}

pub fn start_controller_server_until<F>(
    config: ControllerServerConfig,
    should_shutdown: F,
) -> Result<(), ControllerError>
where
    F: Fn() -> bool + Send + Sync + 'static,
{
    validate_transport(&config)?;
    let db_path = config
        .database_path
        .clone()
        .unwrap_or_else(|| config.data_dir.join("controller").join("fleet.db"));
    let store = SqliteStore::open(db_path)?;
    let identity = load_controller_identity(&config.data_dir)?;

    tracing::info!(
        bind_addr = %format!("{}:{}", config.host, config.port),
        controller_fingerprint = %identity.fingerprint,
        dev_insecure_loopback = config.dev_insecure_loopback,
        "controller_started"
    );
    if config.dev_insecure_loopback {
        tracing::warn!(
            bind_host = %config.host,
            "dev_insecure_loopback_enabled"
        );
        audit_dev_insecure_loopback_enabled(&store, &config.host)?;
    }
    println!("controller listening on {}:{}", config.host, config.port);
    if config.dev_insecure_loopback {
        println!("warning: dev insecure loopback mode enabled");
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run_axum_controller_server(
        config,
        store,
        identity,
        should_shutdown,
    ))
}

async fn run_axum_controller_server<F>(
    config: ControllerServerConfig,
    store: SqliteStore,
    identity: ControllerIdentity,
    should_shutdown: F,
) -> Result<(), ControllerError>
where
    F: Fn() -> bool + Send + Sync + 'static,
{
    let state = ControllerAppState {
        store: Arc::new(Mutex::new(store)),
        identity: Arc::new(identity),
    };
    let app = Router::new()
        .route("/api/agents/ws", get(axum_agent_websocket))
        .fallback(axum_http_fallback)
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.host, config.port))
        .await
        .map_err(ControllerError::Io)?;

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            while !should_shutdown() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .map_err(ControllerError::Io)?;

    tracing::info!("controller_stopped");
    Ok(())
}

async fn axum_http_fallback(
    State(state): State<ControllerAppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> AxumResponse {
    let request = raw_http_request_from_axum(method, uri, headers, body);
    let result = state
        .store
        .lock()
        .map_err(|_| {
            ControllerError::Store(fleet_store::StoreError::Domain(
                "store lock poisoned".to_owned(),
            ))
        })
        .and_then(|store| route_request_with_identity(&request, &store, &state.identity));

    match result {
        Ok(response) => axum_response_from_raw(&response),
        Err(error) => {
            tracing::warn!(error = %error, "controller_request_failed");
            axum_response_from_raw(&response(
                500,
                "application/json",
                &format!("{{\"error\":\"{}\"}}\n", json_escape(&error.to_string())),
            ))
        }
    }
}

async fn axum_agent_websocket(
    State(state): State<ControllerAppState>,
    websocket: WebSocketUpgrade,
) -> impl IntoResponse {
    websocket.on_upgrade(move |socket| async move {
        if let Err(error) = handle_agent_websocket_axum(socket, state).await {
            tracing::warn!(error = %error, "controller_websocket_failed");
        }
    })
}

async fn handle_agent_websocket_axum(
    mut socket: WebSocket,
    state: ControllerAppState,
) -> Result<(), ControllerError> {
    let agent_hello = read_axum_wire_message(&mut socket).await?;
    let fleet_protocol::WirePayload::AgentHello {
        agent_id,
        fingerprint,
    } = agent_hello.payload
    else {
        let store = lock_store(&state)?;
        audit_security(&store, "websocket_expected_agent_hello", "unknown")?;
        return Ok(());
    };

    let Some(public_key) = ({
        let store = lock_store(&state)?;
        validate_agent_ws_hello(&store, &agent_id, &fingerprint)?
    }) else {
        return Ok(());
    };

    let nonce = generate_token("challenge")?;
    let challenge = fleet_protocol::WireMessage::new(
        fleet_core::generate_prefixed_ulid("msg")
            .map_err(|error| ControllerError::Json(error.to_string()))?,
        agent_hello.correlation_id.0,
        Some(agent_id.clone()),
        epoch_millis() as u64,
        fleet_protocol::WirePayload::AuthChallenge {
            nonce: nonce.clone(),
        },
    );
    send_axum_wire_message(&mut socket, &challenge).await?;

    let auth_response = read_axum_wire_message(&mut socket).await?;
    let fleet_protocol::WirePayload::AuthResponse {
        nonce: seen_nonce,
        signature,
    } = &auth_response.payload
    else {
        let store = lock_store(&state)?;
        audit_security(&store, "websocket_expected_auth_response", &agent_id)?;
        return Ok(());
    };

    if !verify_agent_auth_response(&public_key, &nonce, seen_nonce, signature) {
        let store = lock_store(&state)?;
        audit_security(&store, "websocket_invalid_signature", &agent_id)?;
        return Ok(());
    }

    let accepted = fleet_protocol::WireMessage::new(
        fleet_core::generate_prefixed_ulid("msg")
            .map_err(|error| ControllerError::Json(error.to_string()))?,
        auth_response.correlation_id.0,
        Some(agent_id.clone()),
        epoch_millis() as u64,
        fleet_protocol::WirePayload::AuthAccepted,
    );
    send_axum_wire_message(&mut socket, &accepted).await?;

    let heartbeat = read_axum_wire_message(&mut socket).await?;
    if let fleet_protocol::WirePayload::Heartbeat {
        agent_id: heartbeat_agent_id,
        ..
    } = heartbeat.payload
        && heartbeat_agent_id == agent_id
    {
        let assignment = {
            let store = lock_store(&state)?;
            store.mark_agent_online(&agent_id, SystemTime::now())?;
            pending_task_assignment_message(&store, &agent_id)?
        };
        let dispatched = assignment.is_some();
        if let Some(message) = assignment {
            send_axum_wire_message(&mut socket, &message).await?;
        }
        read_task_data_until_close_axum(&mut socket, &state, &agent_id, !dispatched).await?;
        let _ = socket.send(AxumWsMessage::Close(None)).await;
    } else {
        let store = lock_store(&state)?;
        audit_security(&store, "websocket_invalid_heartbeat", &agent_id)?;
    }

    Ok(())
}

async fn read_task_data_until_close_axum(
    socket: &mut WebSocket,
    state: &ControllerAppState,
    agent_id: &str,
    stop_after_first_message: bool,
) -> Result<(), ControllerError> {
    let mut handled_messages = 0usize;
    loop {
        let message = match read_axum_wire_message(socket).await {
            Ok(message) => message,
            Err(ControllerError::Json(error)) if error == "websocket closed" => return Ok(()),
            Err(error) => return Err(error),
        };
        let done = {
            let store = lock_store(state)?;
            handle_agent_task_data_message(&store, agent_id, message)?
        };
        handled_messages += 1;
        if done || (stop_after_first_message && handled_messages >= 2) {
            return Ok(());
        }
    }
}

async fn read_axum_wire_message(
    socket: &mut WebSocket,
) -> Result<fleet_protocol::WireMessage, ControllerError> {
    loop {
        let Some(message) = socket.recv().await else {
            return Err(ControllerError::Json("websocket closed".to_owned()));
        };
        match message.map_err(|error| ControllerError::Json(error.to_string()))? {
            AxumWsMessage::Text(body) => {
                return fleet_protocol::decode_message(&body).map_err(ControllerError::from);
            }
            AxumWsMessage::Close(_) => {
                return Err(ControllerError::Json("websocket closed".to_owned()));
            }
            AxumWsMessage::Binary(_) | AxumWsMessage::Ping(_) | AxumWsMessage::Pong(_) => {}
        }
    }
}

async fn send_axum_wire_message(
    socket: &mut WebSocket,
    message: &fleet_protocol::WireMessage,
) -> Result<(), ControllerError> {
    socket
        .send(AxumWsMessage::Text(
            fleet_protocol::encode_message(message)?.into(),
        ))
        .await
        .map_err(|error| ControllerError::Json(error.to_string()))
}

fn pending_task_assignment_message(
    store: &SqliteStore,
    agent_id: &str,
) -> Result<Option<fleet_protocol::WireMessage>, ControllerError> {
    if let Some(assignment) = store
        .list_pending_command_assignments_for_agent(agent_id)?
        .into_iter()
        .next()
    {
        store.update_job_status(assignment.envelope.job_id.as_str(), JobStatus::Running)?;
        audit_job(
            store,
            "job_started",
            assignment.envelope.job_id.as_str(),
            AuditValue::Plain(format!("agent_id={agent_id}")),
        )?;
        return Ok(Some(fleet_protocol::WireMessage::new(
            fleet_core::generate_prefixed_ulid("msg")
                .map_err(|error| ControllerError::Json(error.to_string()))?,
            assignment.envelope.task_id.as_str().to_owned(),
            Some(agent_id.to_owned()),
            epoch_millis() as u64,
            fleet_protocol::WirePayload::TaskAssignment {
                envelope: task_envelope_to_wire(&assignment.envelope),
                task: command_task_to_wire(&assignment.command),
            },
        )));
    }

    if let Some(assignment) = store
        .list_pending_runbook_assignments_for_agent(agent_id)?
        .into_iter()
        .next()
    {
        store.update_job_status(assignment.envelope.job_id.as_str(), JobStatus::Running)?;
        audit_job(
            store,
            "runbook_job_started",
            assignment.envelope.job_id.as_str(),
            AuditValue::Plain(format!("agent_id={agent_id}")),
        )?;
        return Ok(Some(fleet_protocol::WireMessage::new(
            fleet_core::generate_prefixed_ulid("msg")
                .map_err(|error| ControllerError::Json(error.to_string()))?,
            assignment.envelope.task_id.as_str().to_owned(),
            Some(agent_id.to_owned()),
            epoch_millis() as u64,
            fleet_protocol::WirePayload::TaskAssignment {
                envelope: task_envelope_to_wire(&assignment.envelope),
                task: fleet_protocol::TaskWire::RunbookExecution(
                    fleet_protocol::RunbookExecutionTaskWire {
                        runbook_document: assignment.runbook.runbook_document().to_owned(),
                        timeout_ms: assignment.runbook.timeout().as_millis() as u64,
                        confirmed_high_risk: true,
                    },
                ),
            },
        )));
    }

    let Some(assignment) = store
        .list_pending_drift_check_assignments_for_agent(agent_id)?
        .into_iter()
        .next()
    else {
        return Ok(None);
    };
    store.update_job_status(assignment.envelope.job_id.as_str(), JobStatus::Running)?;
    audit_job(
        store,
        "drift_check_job_started",
        assignment.envelope.job_id.as_str(),
        AuditValue::Plain(format!("agent_id={agent_id}")),
    )?;
    Ok(Some(fleet_protocol::WireMessage::new(
        fleet_core::generate_prefixed_ulid("msg")
            .map_err(|error| ControllerError::Json(error.to_string()))?,
        assignment.envelope.task_id.as_str().to_owned(),
        Some(agent_id.to_owned()),
        epoch_millis() as u64,
        fleet_protocol::WirePayload::TaskAssignment {
            envelope: task_envelope_to_wire(&assignment.envelope),
            task: fleet_protocol::TaskWire::DriftCheck(fleet_protocol::DriftCheckTaskWire {
                policy_document: assignment.drift_check.policy_document().to_owned(),
            }),
        },
    )))
}

fn lock_store(
    state: &ControllerAppState,
) -> Result<std::sync::MutexGuard<'_, SqliteStore>, ControllerError> {
    state.store.lock().map_err(|_| {
        ControllerError::Store(fleet_store::StoreError::Domain(
            "store lock poisoned".to_owned(),
        ))
    })
}

fn raw_http_request_from_axum(method: Method, uri: Uri, headers: HeaderMap, body: Bytes) -> String {
    let target = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or(uri.path());
    let mut request = format!("{method} {target} HTTP/1.1\r\n");
    let mut has_content_length = false;
    for (name, value) in &headers {
        if name.as_str().eq_ignore_ascii_case("content-length") {
            has_content_length = true;
        }
        if let Ok(value) = value.to_str() {
            request.push_str(name.as_str());
            request.push_str(": ");
            request.push_str(value);
            request.push_str("\r\n");
        }
    }
    if !has_content_length {
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    request.push_str("\r\n");
    request.push_str(&String::from_utf8_lossy(&body));
    request
}

fn axum_response_from_raw(raw: &str) -> AxumResponse {
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw, ""));
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .and_then(|code| StatusCode::from_u16(code).ok())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut response = body.to_owned().into_response();
    *response.status_mut() = status;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") || name.eq_ignore_ascii_case("connection") {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.trim().as_bytes()),
            HeaderValue::from_str(value.trim()),
        ) {
            response.headers_mut().insert(name, value);
        }
    }
    response
}

pub fn create_admin_token(store: &SqliteStore) -> Result<Option<String>, ControllerError> {
    let mut repo = ControllerAdminTokenRepository { store };
    let token = generate_token("admin")?;
    let created = EnsureAdminToken::execute(&mut repo, &hash_token(&token))?;
    if !created {
        return Ok(None);
    }
    Ok(Some(token))
}

fn load_controller_identity(data_dir: &Path) -> Result<ControllerIdentity, ControllerError> {
    let public_key_path = data_dir.join("controller").join("controller_public.key");
    let private_key_path = data_dir.join("controller").join("controller_private.key");
    let public_key = std::fs::read_to_string(public_key_path)?.trim().to_owned();
    let private_key = std::fs::read_to_string(private_key_path)?.trim().to_owned();
    let fingerprint = fleet_core::fingerprint_public_key(&public_key)
        .map_err(|error| ControllerError::Json(error.to_string()))?;
    Ok(ControllerIdentity {
        public_key,
        fingerprint,
        private_key,
    })
}

fn validate_agent_ws_hello(
    store: &SqliteStore,
    agent_id: &str,
    fingerprint: &str,
) -> Result<Option<String>, ControllerError> {
    let Some((public_key, stored_fingerprint)) = store.find_agent_identity(agent_id)? else {
        audit_security(store, "websocket_unknown_agent", agent_id)?;
        return Ok(None);
    };
    if stored_fingerprint != fingerprint {
        audit_security(store, "websocket_fingerprint_mismatch", agent_id)?;
        return Ok(None);
    }
    Ok(Some(public_key))
}

fn verify_agent_auth_response(
    public_key: &str,
    expected_nonce: &str,
    seen_nonce: &str,
    signature: &str,
) -> bool {
    seen_nonce == expected_nonce
        && fleet_core::verify_challenge_signature(public_key, expected_nonce, signature)
            .unwrap_or(false)
}

fn handle_agent_task_data_message(
    store: &SqliteStore,
    agent_id: &str,
    message: fleet_protocol::WireMessage,
) -> Result<bool, ControllerError> {
    match message.payload {
        fleet_protocol::WirePayload::OutputChunk {
            job_id,
            task_id: _,
            stream,
            sequence,
            data,
        } => store.append_job_output_chunk_record(&JobOutputChunk {
            job_id,
            agent_id: agent_id.to_owned(),
            stream: output_stream_from_wire(stream),
            sequence,
            body: data,
        })?,
        fleet_protocol::WirePayload::TaskResult {
            job_id,
            task_id: _,
            exit_code,
        } => {
            let status = if exit_code == 0 {
                JobStatus::Success
            } else {
                JobStatus::Failed
            };
            store.update_job_status(&job_id, status)?;
            audit_job(
                store,
                if exit_code == 0 {
                    "job_completed"
                } else {
                    "job_failed"
                },
                &job_id,
                AuditValue::Plain(format!("agent_id={agent_id},exit_code={exit_code}")),
            )?;
            return Ok(true);
        }
        fleet_protocol::WirePayload::SecurityEvent {
            agent_id: event_agent_id,
            action,
            detail,
        } => {
            if event_agent_id != agent_id {
                audit_security(store, "websocket_security_event_agent_mismatch", agent_id)?;
            } else {
                audit_security_with_value(
                    store,
                    &action,
                    agent_id,
                    AuditValue::Plain(format!("detail={detail}")),
                )?;
            }
            return Ok(true);
        }
        fleet_protocol::WirePayload::FactsSnapshot {
            agent_id: event_agent_id,
            body,
        } => {
            if event_agent_id != agent_id {
                audit_security(store, "websocket_facts_agent_mismatch", agent_id)?;
            } else {
                if facts_payload_is_degraded(&body) {
                    store.mark_agent_degraded(agent_id, SystemTime::now())?;
                }
                store.insert_facts_snapshot(agent_id, &body, SystemTime::now())?;
            }
        }
        fleet_protocol::WirePayload::MetricsSnapshot {
            agent_id: event_agent_id,
            body,
        } => {
            if event_agent_id != agent_id {
                audit_security(store, "websocket_metrics_agent_mismatch", agent_id)?;
            } else {
                store.insert_metrics_snapshot(agent_id, &body, SystemTime::now())?;
            }
        }
        fleet_protocol::WirePayload::DriftReport {
            agent_id: event_agent_id,
            status,
            expected,
            actual,
        } => {
            if event_agent_id != agent_id {
                audit_security(store, "websocket_drift_agent_mismatch", agent_id)?;
            } else {
                let report = DriftReport {
                    policy_name: "agent-reported".to_owned(),
                    status: parse_drift_status(&status),
                    expected,
                    actual,
                };
                store.insert_drift_report(agent_id, &report, SystemTime::now())?;
                audit_drift(
                    store,
                    "drift_report_received",
                    agent_id,
                    AuditValue::Plain(format!(
                        "policy_name={},status={}",
                        report.policy_name,
                        drift_status_to_str(&report.status)
                    )),
                )?;
            }
        }
        _ => audit_security(store, "websocket_unexpected_task_data", agent_id)?,
    }
    Ok(false)
}

fn task_envelope_to_wire(envelope: &TaskEnvelope) -> fleet_protocol::SignedTaskEnvelopeWire {
    fleet_protocol::SignedTaskEnvelopeWire {
        job_id: envelope.job_id.as_str().to_owned(),
        task_id: envelope.task_id.as_str().to_owned(),
        target_agent_id: envelope.target_agent_id.as_str().to_owned(),
        issued_at_ms: system_time_to_millis(envelope.issued_at),
        expires_at_ms: system_time_to_millis(envelope.expires_at.as_system_time()),
        nonce: envelope.nonce.as_str().to_owned(),
        payload_hash: envelope.payload_hash.clone(),
        signature: envelope
            .signature
            .as_ref()
            .map(|signature| signature.as_str().to_owned())
            .unwrap_or_default(),
    }
}

fn command_task_to_wire(command: &fleet_domain::CommandTask) -> fleet_protocol::TaskWire {
    fleet_protocol::TaskWire::Command(fleet_protocol::CommandTaskWire {
        program: command.program().to_owned(),
        args: command.args().to_vec(),
        timeout_ms: command.timeout().as_millis() as u64,
        max_output_bytes: command.max_output_bytes(),
    })
}

fn output_stream_from_wire(stream: fleet_protocol::OutputStream) -> JobOutputStream {
    match stream {
        fleet_protocol::OutputStream::Stdout => JobOutputStream::Stdout,
        fleet_protocol::OutputStream::Stderr => JobOutputStream::Stderr,
    }
}

fn audit_security(store: &SqliteStore, action: &str, target: &str) -> Result<(), ControllerError> {
    store.write_audit_event(AuditEvent::security(action, target))?;
    Ok(())
}

fn audit_security_with_value(
    store: &SqliteStore,
    action: &str,
    target: &str,
    value: AuditValue,
) -> Result<(), ControllerError> {
    store.write_audit_event(AuditEvent {
        category: AuditCategory::Security,
        action: action.to_owned(),
        actor: AuditActor::new("agent"),
        target: AuditTarget::new(target),
        value,
        occurred_at: SystemTime::now(),
    })?;
    Ok(())
}

fn audit_dev_insecure_loopback_enabled(
    store: &SqliteStore,
    host: &str,
) -> Result<(), ControllerError> {
    store.write_audit_event(AuditEvent {
        category: AuditCategory::Security,
        action: "dev_insecure_loopback_enabled".to_owned(),
        actor: AuditActor::new("controller"),
        target: AuditTarget::new(host),
        value: AuditValue::Plain("loopback_only".to_owned()),
        occurred_at: SystemTime::now(),
    })?;
    Ok(())
}

fn audit_job(
    store: &SqliteStore,
    action: &str,
    target: &str,
    value: AuditValue,
) -> Result<(), ControllerError> {
    store.write_audit_event(AuditEvent {
        category: AuditCategory::Job,
        action: action.to_owned(),
        actor: AuditActor::new("controller"),
        target: AuditTarget::new(target),
        value,
        occurred_at: SystemTime::now(),
    })?;
    Ok(())
}

fn audit_drift(
    store: &SqliteStore,
    action: &str,
    target: &str,
    value: AuditValue,
) -> Result<(), ControllerError> {
    store.write_audit_event(AuditEvent {
        category: AuditCategory::Drift,
        action: action.to_owned(),
        actor: AuditActor::new("agent"),
        target: AuditTarget::new(target),
        value,
        occurred_at: SystemTime::now(),
    })?;
    Ok(())
}

#[cfg(test)]
fn route_request(request: &str, store: &SqliteStore) -> Result<String, ControllerError> {
    route_request_with_identity(request, store, &ControllerIdentity::dev_insecure())
}

fn route_request_with_identity(
    request: &str,
    store: &SqliteStore,
    identity: &ControllerIdentity,
) -> Result<String, ControllerError> {
    let Some(request_line) = request.lines().next() else {
        return Ok(response(400, "text/plain", "bad request\n"));
    };
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();

    if method == "GET" && path == "/healthz" {
        return Ok(response(200, "application/json", "{\"status\":\"ok\"}\n"));
    }

    if method == "GET" && path == "/api/controller/identity" {
        let body = serde_json::to_string(&ControllerIdentityResponse {
            controller_public_key: identity.public_key.clone(),
            controller_fingerprint: identity.fingerprint.clone(),
        })
        .map_err(|error| ControllerError::Json(error.to_string()))?;
        return Ok(response(200, "application/json", &format!("{body}\n")));
    }

    if method == "GET" && path.starts_with("/admin") {
        return Ok(admin_static_response(path));
    }

    if path.starts_with("/api/") && path != "/api/agents/enroll" && !authorized(request, store)? {
        return Ok(response(
            401,
            "application/json",
            "{\"error\":\"unauthorized\"}\n",
        ));
    }

    match (method, path) {
        ("POST", "/api/agents/enroll") => {
            match enroll_agent(request_body(request), store, identity) {
                Ok(body) => Ok(response(201, "application/json", &format!("{body}\n"))),
                Err(ControllerError::Store(fleet_store::StoreError::NotFound)) => Ok(response(
                    401,
                    "application/json",
                    "{\"error\":\"invalid_enrollment_token\"}\n",
                )),
                Err(ControllerError::Store(fleet_store::StoreError::Domain(message))) => {
                    Ok(response(
                        400,
                        "application/json",
                        &format!("{{\"error\":\"{}\"}}\n", json_escape(&message)),
                    ))
                }
                Err(ControllerError::Store(fleet_store::StoreError::DuplicateAgent)) => Ok(
                    response(409, "application/json", "{\"error\":\"duplicate_agent\"}\n"),
                ),
                Err(ControllerError::Store(fleet_store::StoreError::ConstraintViolation(_))) => {
                    Ok(response(
                        409,
                        "application/json",
                        "{\"error\":\"duplicate_or_constraint_violation\"}\n",
                    ))
                }
                Err(error) => Err(error),
            }
        }
        ("POST", "/api/enrollment-tokens") => {
            let body = create_enrollment_token(store)?;
            Ok(response(201, "application/json", &format!("{body}\n")))
        }
        ("POST", "/api/jobs/command") => {
            match create_command_job(request_body(request), store, identity) {
                Ok(body) => Ok(response(201, "application/json", &format!("{body}\n"))),
                Err(CreateCommandJobHttpError::BadRequest(message)) => Ok(response(
                    400,
                    "application/json",
                    &format!("{{\"error\":\"{}\"}}\n", json_escape(&message)),
                )),
                Err(CreateCommandJobHttpError::Conflict(message)) => Ok(response(
                    409,
                    "application/json",
                    &format!("{{\"error\":\"{}\"}}\n", json_escape(&message)),
                )),
                Err(CreateCommandJobHttpError::Internal(error)) => Err(error),
            }
        }
        ("POST", "/api/jobs/drift-check") => {
            match create_drift_check_job(request_body(request), store, identity) {
                Ok(body) => Ok(response(201, "application/json", &format!("{body}\n"))),
                Err(CreateCommandJobHttpError::BadRequest(message)) => Ok(response(
                    400,
                    "application/json",
                    &format!("{{\"error\":\"{}\"}}\n", json_escape(&message)),
                )),
                Err(CreateCommandJobHttpError::Conflict(message)) => Ok(response(
                    409,
                    "application/json",
                    &format!("{{\"error\":\"{}\"}}\n", json_escape(&message)),
                )),
                Err(CreateCommandJobHttpError::Internal(error)) => Err(error),
            }
        }
        ("POST", "/api/jobs/runbook") => {
            match create_runbook_job(request_body(request), store, identity) {
                Ok(body) => Ok(response(201, "application/json", &format!("{body}\n"))),
                Err(CreateCommandJobHttpError::BadRequest(message)) => Ok(response(
                    400,
                    "application/json",
                    &format!("{{\"error\":\"{}\"}}\n", json_escape(&message)),
                )),
                Err(CreateCommandJobHttpError::Conflict(message)) => Ok(response(
                    409,
                    "application/json",
                    &format!("{{\"error\":\"{}\"}}\n", json_escape(&message)),
                )),
                Err(CreateCommandJobHttpError::Internal(error)) => Err(error),
            }
        }
        ("GET", "/api/jobs") => {
            let body = list_jobs(store)?;
            Ok(response(200, "application/json", &format!("{body}\n")))
        }
        ("GET", path) if path.starts_with("/api/jobs/") && path.ends_with("/output") => {
            let job_id = path
                .trim_start_matches("/api/jobs/")
                .trim_end_matches("/output")
                .trim_end_matches('/');
            let body = list_job_output(job_id, store)?;
            Ok(response(200, "application/json", &format!("{body}\n")))
        }
        ("GET", "/api/agents") => {
            let body = list_agents(store)?;
            Ok(response(200, "application/json", &format!("{body}\n")))
        }
        ("GET", "/api/audit") => {
            let body = list_audit_events(store)?;
            Ok(response(200, "application/json", &format!("{body}\n")))
        }
        ("GET", path) if path.starts_with("/api/agents/") && path.ends_with("/facts/latest") => {
            let agent_id = path
                .trim_start_matches("/api/agents/")
                .trim_end_matches("/facts/latest")
                .trim_end_matches('/');
            match latest_facts(agent_id, store)? {
                Some(body) => Ok(response(200, "application/json", &format!("{body}\n"))),
                None => Ok(response(
                    404,
                    "application/json",
                    "{\"error\":\"not_found\"}\n",
                )),
            }
        }
        ("GET", path) if path.starts_with("/api/agents/") && path.ends_with("/metrics/latest") => {
            let agent_id = path
                .trim_start_matches("/api/agents/")
                .trim_end_matches("/metrics/latest")
                .trim_end_matches('/');
            match latest_metrics(agent_id, store)? {
                Some(body) => Ok(response(200, "application/json", &format!("{body}\n"))),
                None => Ok(response(
                    404,
                    "application/json",
                    "{\"error\":\"not_found\"}\n",
                )),
            }
        }
        ("GET", path) if path.starts_with("/api/agents/") && path.ends_with("/drift/latest") => {
            let agent_id = path
                .trim_start_matches("/api/agents/")
                .trim_end_matches("/drift/latest")
                .trim_end_matches('/');
            match latest_drift_report(agent_id, store)? {
                Some(body) => Ok(response(200, "application/json", &format!("{body}\n"))),
                None => Ok(response(
                    404,
                    "application/json",
                    "{\"error\":\"not_found\"}\n",
                )),
            }
        }
        ("GET", path) if path.starts_with("/api/agents/") && path != "/api/agents/ws" => {
            let agent_id = path.trim_start_matches("/api/agents/");
            match get_agent(agent_id, store)? {
                Some(body) => Ok(response(200, "application/json", &format!("{body}\n"))),
                None => Ok(response(
                    404,
                    "application/json",
                    "{\"error\":\"not_found\"}\n",
                )),
            }
        }
        ("PATCH", path) if path.starts_with("/api/agents/") && path.ends_with("/labels") => {
            let agent_id = path
                .trim_start_matches("/api/agents/")
                .trim_end_matches("/labels")
                .trim_end_matches('/');
            match update_agent_labels(agent_id, request_body(request), store) {
                Ok(Some(body)) => Ok(response(200, "application/json", &format!("{body}\n"))),
                Ok(None) => Ok(response(
                    404,
                    "application/json",
                    "{\"error\":\"not_found\"}\n",
                )),
                Err(ControllerError::Store(fleet_store::StoreError::Domain(message))) => {
                    Ok(response(
                        400,
                        "application/json",
                        &format!("{{\"error\":\"{}\"}}\n", json_escape(&message)),
                    ))
                }
                Err(ControllerError::Json(message)) => Ok(response(
                    400,
                    "application/json",
                    &format!("{{\"error\":\"{}\"}}\n", json_escape(&message)),
                )),
                Err(error) => Err(error),
            }
        }
        ("GET", "/api/enrollment-tokens") => {
            let body = list_enrollment_tokens(store)?;
            Ok(response(200, "application/json", &format!("[{body}]\n")))
        }
        ("DELETE", path) if path.starts_with("/api/enrollment-tokens/") => {
            let id = path.trim_start_matches("/api/enrollment-tokens/");
            if revoke_enrollment_token(id, store)? {
                Ok(response(204, "application/json", ""))
            } else {
                Ok(response(
                    404,
                    "application/json",
                    "{\"error\":\"not_found\"}\n",
                ))
            }
        }
        _ => Ok(response(
            404,
            "application/json",
            "{\"error\":\"not_found\"}\n",
        )),
    }
}

fn admin_static_response(path: &str) -> String {
    let path = path.split_once('?').map(|(path, _)| path).unwrap_or(path);
    match path {
        "/admin" | "/admin/" | "/admin/index.html" => {
            response(200, "text/html; charset=utf-8", ADMIN_INDEX_HTML)
        }
        "/admin/styles.css" => response(200, "text/css; charset=utf-8", ADMIN_STYLES_CSS),
        "/admin/app.js" => response(200, "application/javascript; charset=utf-8", ADMIN_APP_JS),
        "/admin/api-client.js" => response(
            200,
            "application/javascript; charset=utf-8",
            ADMIN_API_CLIENT_JS,
        ),
        "/admin/api.schema.json" => response(
            200,
            "application/json; charset=utf-8",
            ADMIN_API_SCHEMA_JSON,
        ),
        _ => response(404, "application/json", "{\"error\":\"not_found\"}\n"),
    }
}

enum CreateCommandJobHttpError {
    BadRequest(String),
    Conflict(String),
    Internal(ControllerError),
}

fn create_enrollment_token(store: &SqliteStore) -> Result<String, ControllerError> {
    let id = fleet_core::generate_prefixed_ulid("et")
        .map_err(|error| ControllerError::Json(error.to_string()))?;
    let token = generate_token("enroll")?;
    let expires_in_seconds = 3600;
    let now = SystemTime::now();
    let mut repo = ControllerEnrollmentTokenRepository { store };
    let mut audit = ControllerAuditWriter { store };
    let output = CreateEnrollmentToken::execute(
        &mut repo,
        &mut audit,
        CreateEnrollmentTokenInput {
            id,
            token_hash: hash_token(&token),
            default_labels: String::new(),
            expires_at: now + Duration::from_secs(expires_in_seconds),
            max_uses: 1,
            occurred_at: now,
        },
    )
    .map_err(map_enrollment_token_use_case_error)?;

    serde_json::to_string(&CreateEnrollmentTokenResponse {
        id: output.id,
        token,
        expires_in_seconds,
    })
    .map_err(|error| ControllerError::Json(error.to_string()))
}

fn list_enrollment_tokens(store: &SqliteStore) -> Result<String, ControllerError> {
    let repo = ControllerEnrollmentTokenRepository { store };
    let records = ListEnrollmentTokens::execute(&repo).map_err(ControllerError::Store)?;
    Ok(records
        .into_iter()
        .map(|record| {
            serde_json::to_string(&EnrollmentTokenSummaryResponse {
                id: record.id,
                default_labels: record.default_labels,
                max_uses: record.max_uses,
                used_count: record.used_count,
                revoked: record.revoked,
            })
            .map_err(|error| ControllerError::Json(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?
        .join(","))
}

fn revoke_enrollment_token(id: &str, store: &SqliteStore) -> Result<bool, ControllerError> {
    let mut repo = ControllerEnrollmentTokenRepository { store };
    let mut audit = ControllerAuditWriter { store };
    let output = RevokeEnrollmentToken::execute(
        &mut repo,
        &mut audit,
        RevokeEnrollmentTokenInput {
            id: id.to_owned(),
            occurred_at: SystemTime::now(),
        },
    )
    .map_err(map_enrollment_token_use_case_error)?;
    Ok(output.revoked)
}

fn map_enrollment_token_use_case_error(
    error: EnrollmentTokenUseCaseError<fleet_store::StoreError, fleet_store::StoreError>,
) -> ControllerError {
    match error {
        EnrollmentTokenUseCaseError::Repository(error)
        | EnrollmentTokenUseCaseError::Audit(error) => ControllerError::Store(error),
    }
}

fn create_command_job(
    body: &str,
    store: &SqliteStore,
    identity: &ControllerIdentity,
) -> Result<String, CreateCommandJobHttpError> {
    let request: CreateCommandJobRequest = serde_json::from_str(body)
        .map_err(|error| CreateCommandJobHttpError::BadRequest(error.to_string()))?;
    let issued_at = SystemTime::now();
    let expires_at = issued_at + Duration::from_secs(request.expires_in_seconds);
    let job_id = request.job_id.clone();
    let target_agent_ids = resolve_command_targets(store, &request)?;
    let nonce_prefix = match request.nonce_prefix {
        Some(prefix) => prefix,
        None => fleet_core::generate_prefixed_ulid("nonce").map_err(|error| {
            CreateCommandJobHttpError::Internal(ControllerError::Json(error.to_string()))
        })?,
    };
    let input = CreateCommandJobInput {
        job_id: request.job_id,
        target_agent_ids,
        program: request.program,
        args: request.args,
        timeout: Duration::from_secs(request.timeout_seconds),
        confirmed_high_risk: request.confirmed_high_risk,
        confirmed_by: request.confirmed_by,
        issued_at,
        expires_at,
        nonce_prefix,
    };
    let mut job_repo = ControllerJobRepository { store };
    let mut audit_writer = ControllerAuditWriter { store };
    let mut signer = ControllerTaskSigner {
        private_key: &identity.private_key,
    };

    let output = CreateCommandJob::execute(&mut job_repo, &mut audit_writer, &mut signer, input)
        .map_err(map_create_command_job_error)?;
    serde_json::to_string(&CreateCommandJobResponse {
        job_id,
        target_count: output.targets.len(),
        assignment_count: output.envelopes.len(),
    })
    .map_err(|error| CreateCommandJobHttpError::Internal(ControllerError::Json(error.to_string())))
}

fn create_drift_check_job(
    body: &str,
    store: &SqliteStore,
    identity: &ControllerIdentity,
) -> Result<String, CreateCommandJobHttpError> {
    let request: CreateDriftCheckJobRequest = serde_json::from_str(body)
        .map_err(|error| CreateCommandJobHttpError::BadRequest(error.to_string()))?;
    fleet_domain::parse_policy_document(&request.policy_document)
        .map_err(|error| CreateCommandJobHttpError::BadRequest(error.to_string()))?;
    let issued_at = SystemTime::now();
    let expires_at = issued_at + Duration::from_secs(request.expires_in_seconds);
    let job_id = request.job_id.clone();
    let target_agent_ids = resolve_drift_check_targets(store, &request)?;
    let nonce_prefix = match request.nonce_prefix {
        Some(prefix) => prefix,
        None => fleet_core::generate_prefixed_ulid("nonce").map_err(|error| {
            CreateCommandJobHttpError::Internal(ControllerError::Json(error.to_string()))
        })?,
    };
    let input = CreateDriftCheckJobInput {
        job_id: request.job_id,
        target_agent_ids,
        policy_document: request.policy_document,
        timeout: Duration::from_secs(request.timeout_seconds),
        created_by: request.created_by,
        issued_at,
        expires_at,
        nonce_prefix,
    };
    let mut job_repo = ControllerJobRepository { store };
    let mut audit_writer = ControllerAuditWriter { store };
    let mut signer = ControllerTaskSigner {
        private_key: &identity.private_key,
    };

    let output = CreateDriftCheckJob::execute(&mut job_repo, &mut audit_writer, &mut signer, input)
        .map_err(map_create_drift_check_job_error)?;
    serde_json::to_string(&CreateDriftCheckJobResponse {
        job_id,
        target_count: output.targets.len(),
        assignment_count: output.envelopes.len(),
    })
    .map_err(|error| CreateCommandJobHttpError::Internal(ControllerError::Json(error.to_string())))
}

fn create_runbook_job(
    body: &str,
    store: &SqliteStore,
    identity: &ControllerIdentity,
) -> Result<String, CreateCommandJobHttpError> {
    let request: CreateRunbookJobRequest = serde_json::from_str(body)
        .map_err(|error| CreateCommandJobHttpError::BadRequest(error.to_string()))?;
    fleet_domain::parse_runbook_document(&request.runbook_document)
        .map_err(|error| CreateCommandJobHttpError::BadRequest(error.to_string()))?;
    let issued_at = SystemTime::now();
    let expires_at = issued_at + Duration::from_secs(request.expires_in_seconds);
    let job_id = request.job_id.clone();
    let target_agent_ids = resolve_runbook_targets(store, &request)?;
    let nonce_prefix = match request.nonce_prefix {
        Some(prefix) => prefix,
        None => fleet_core::generate_prefixed_ulid("nonce").map_err(|error| {
            CreateCommandJobHttpError::Internal(ControllerError::Json(error.to_string()))
        })?,
    };
    let input = CreateRunbookJobInput {
        job_id: request.job_id,
        target_agent_ids,
        runbook_document: request.runbook_document,
        timeout: Duration::from_secs(request.timeout_seconds),
        confirmed_high_risk: request.confirmed_high_risk,
        confirmed_by: request.confirmed_by,
        issued_at,
        expires_at,
        nonce_prefix,
    };
    let mut job_repo = ControllerJobRepository { store };
    let mut audit_writer = ControllerAuditWriter { store };
    let mut signer = ControllerTaskSigner {
        private_key: &identity.private_key,
    };

    let output = CreateRunbookJob::execute(&mut job_repo, &mut audit_writer, &mut signer, input)
        .map_err(map_create_runbook_job_error)?;
    serde_json::to_string(&CreateRunbookJobResponse {
        job_id,
        target_count: output.targets.len(),
        assignment_count: output.envelopes.len(),
    })
    .map_err(|error| CreateCommandJobHttpError::Internal(ControllerError::Json(error.to_string())))
}

fn resolve_command_targets(
    store: &SqliteStore,
    request: &CreateCommandJobRequest,
) -> Result<Vec<String>, CreateCommandJobHttpError> {
    if !request.target_agent_ids.is_empty() {
        return Ok(request.target_agent_ids.clone());
    }
    let Some(selector) = request.selector.as_deref() else {
        return Ok(Vec::new());
    };
    let selector = Selector::parse(selector)
        .map_err(|error| CreateCommandJobHttpError::BadRequest(error.to_string()))?;
    let repo = ControllerAgentInventoryRepository { store };
    let agents = ListInventoryAgents::execute(&repo)
        .map_err(|error| CreateCommandJobHttpError::Internal(ControllerError::Store(error)))?;
    let selection = select_dispatch_targets(&agents, &selector);
    tracing::debug!(
        matched_count = selection.matched_count,
        selected_count = selection.targets.len(),
        disabled_count = selection.disabled_count,
        offline_count = selection.offline_count,
        "job_selector_resolved"
    );
    Ok(selection
        .targets
        .into_iter()
        .map(|agent| agent.id().as_str().to_owned())
        .collect())
}

fn resolve_drift_check_targets(
    store: &SqliteStore,
    request: &CreateDriftCheckJobRequest,
) -> Result<Vec<String>, CreateCommandJobHttpError> {
    if !request.target_agent_ids.is_empty() {
        return Ok(request.target_agent_ids.clone());
    }
    let Some(selector) = request.selector.as_deref() else {
        return Ok(Vec::new());
    };
    let selector = Selector::parse(selector)
        .map_err(|error| CreateCommandJobHttpError::BadRequest(error.to_string()))?;
    let repo = ControllerAgentInventoryRepository { store };
    let agents = ListInventoryAgents::execute(&repo)
        .map_err(|error| CreateCommandJobHttpError::Internal(ControllerError::Store(error)))?;
    let selection = select_dispatch_targets(&agents, &selector);
    tracing::debug!(
        matched_count = selection.matched_count,
        selected_count = selection.targets.len(),
        disabled_count = selection.disabled_count,
        offline_count = selection.offline_count,
        "drift_check_selector_resolved"
    );
    Ok(selection
        .targets
        .into_iter()
        .map(|agent| agent.id().as_str().to_owned())
        .collect())
}

fn resolve_runbook_targets(
    store: &SqliteStore,
    request: &CreateRunbookJobRequest,
) -> Result<Vec<String>, CreateCommandJobHttpError> {
    if !request.target_agent_ids.is_empty() {
        return Ok(request.target_agent_ids.clone());
    }
    let Some(selector) = request.selector.as_deref() else {
        return Ok(Vec::new());
    };
    let selector = Selector::parse(selector)
        .map_err(|error| CreateCommandJobHttpError::BadRequest(error.to_string()))?;
    let repo = ControllerAgentInventoryRepository { store };
    let agents = ListInventoryAgents::execute(&repo)
        .map_err(|error| CreateCommandJobHttpError::Internal(ControllerError::Store(error)))?;
    let selection = select_dispatch_targets(&agents, &selector);
    tracing::debug!(
        matched_count = selection.matched_count,
        selected_count = selection.targets.len(),
        disabled_count = selection.disabled_count,
        offline_count = selection.offline_count,
        "runbook_selector_resolved"
    );
    Ok(selection
        .targets
        .into_iter()
        .map(|agent| agent.id().as_str().to_owned())
        .collect())
}

fn list_job_output(job_id: &str, store: &SqliteStore) -> Result<String, ControllerError> {
    let repo = ControllerJobOutputRepository { store };
    let chunks = ListJobOutputForJob::execute(&repo, job_id)?;
    let response = chunks
        .into_iter()
        .map(|chunk| JobOutputChunkResponse {
            job_id: chunk.job_id,
            agent_id: chunk.agent_id,
            stream: job_output_stream_to_str(chunk.stream).to_owned(),
            sequence: chunk.sequence,
            data: chunk.body,
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&response).map_err(|error| ControllerError::Json(error.to_string()))
}

fn job_output_stream_to_str(stream: JobOutputStream) -> &'static str {
    match stream {
        JobOutputStream::Stdout => "stdout",
        JobOutputStream::Stderr => "stderr",
    }
}

fn list_agents(store: &SqliteStore) -> Result<String, ControllerError> {
    let repo = ControllerAgentInventoryRepository { store };
    let agents = ListInventoryAgents::execute(&repo)?
        .iter()
        .map(agent_to_response)
        .collect::<Vec<_>>();
    serde_json::to_string(&agents).map_err(|error| ControllerError::Json(error.to_string()))
}

fn get_agent(agent_id: &str, store: &SqliteStore) -> Result<Option<String>, ControllerError> {
    let agent_id = AgentId::new(agent_id).map_err(|error| ControllerError::Store(error.into()))?;
    let repo = ControllerAgentInventoryRepository { store };
    let Some(agent) = GetInventoryAgent::execute(&repo, agent_id)? else {
        return Ok(None);
    };
    serde_json::to_string(&agent_to_response(&agent))
        .map(Some)
        .map_err(|error| ControllerError::Json(error.to_string()))
}

fn update_agent_labels(
    agent_id: &str,
    body: &str,
    store: &SqliteStore,
) -> Result<Option<String>, ControllerError> {
    let request: UpdateAgentLabelsRequest =
        serde_json::from_str(body).map_err(|error| ControllerError::Json(error.to_string()))?;
    let labels = request
        .labels
        .into_iter()
        .map(|label| {
            AgentLabel::new(label.key, label.value)
                .map_err(|error| ControllerError::Store(error.into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut repo = ControllerAgentInventoryRepository { store };
    let mut audit = ControllerAuditWriter { store };
    let Some(agent) = UpdateAgentLabels::execute(
        &mut repo,
        &mut audit,
        UpdateAgentLabelsInput {
            agent_id: agent_id.to_owned(),
            labels,
            actor: "admin".to_owned(),
            occurred_at: SystemTime::now(),
        },
    )
    .map_err(map_update_agent_labels_error)?
    else {
        return Ok(None);
    };
    serde_json::to_string(&agent_to_response(&agent))
        .map(Some)
        .map_err(|error| ControllerError::Json(error.to_string()))
}

fn latest_facts(agent_id: &str, store: &SqliteStore) -> Result<Option<String>, ControllerError> {
    let repo = ControllerFactsRepository { store };
    let Some(snapshot) = GetLatestFacts::execute(&repo, agent_id)? else {
        return Ok(None);
    };
    let body = serde_json::from_str(&snapshot.body)
        .map_err(|error| ControllerError::Json(error.to_string()))?;
    serde_json::to_string(&LatestFactsResponse {
        agent_id: snapshot.agent_id,
        collected_at_ms: system_time_to_millis(snapshot.collected_at),
        body,
    })
    .map(Some)
    .map_err(|error| ControllerError::Json(error.to_string()))
}

fn list_jobs(store: &SqliteStore) -> Result<String, ControllerError> {
    let repo = ControllerJobQueryRepository { store };
    let jobs = ListJobSummaries::execute(&repo, 50)?;
    let response = jobs
        .into_iter()
        .map(|job| JobSummaryResponse {
            id: job.id,
            status: job.status,
            risk: job.risk,
            command_program: job.command_program,
            command_args: job.command_args,
            target_count: job.target_count,
            created_at_ms: system_time_to_millis(job.created_at),
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&response).map_err(|error| ControllerError::Json(error.to_string()))
}

fn facts_payload_is_degraded(body: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("degraded")
                .and_then(|degraded| degraded.get("status"))
                .and_then(serde_json::Value::as_bool)
        })
        .unwrap_or(false)
}

fn latest_metrics(agent_id: &str, store: &SqliteStore) -> Result<Option<String>, ControllerError> {
    let repo = ControllerMetricsRepository { store };
    let Some(snapshot) = GetLatestMetrics::execute(&repo, agent_id)? else {
        return Ok(None);
    };
    let body = serde_json::from_str(&snapshot.body)
        .map_err(|error| ControllerError::Json(error.to_string()))?;
    serde_json::to_string(&LatestMetricsResponse {
        agent_id: snapshot.agent_id,
        collected_at_ms: system_time_to_millis(snapshot.collected_at),
        body,
    })
    .map(Some)
    .map_err(|error| ControllerError::Json(error.to_string()))
}

fn latest_drift_report(
    agent_id: &str,
    store: &SqliteStore,
) -> Result<Option<String>, ControllerError> {
    let repo = ControllerDriftRepository { store };
    let Some(record) = GetLatestDrift::execute(&repo, agent_id)? else {
        return Ok(None);
    };
    serde_json::to_string(&LatestDriftReportResponse {
        agent_id: record.agent_id,
        checked_at_ms: system_time_to_millis(record.checked_at),
        policy_name: record.report.policy_name,
        status: drift_status_to_str(&record.report.status).to_owned(),
        expected: record.report.expected,
        actual: record.report.actual,
    })
    .map(Some)
    .map_err(|error| ControllerError::Json(error.to_string()))
}

fn list_audit_events(store: &SqliteStore) -> Result<String, ControllerError> {
    let repo = ControllerAuditRepository { store };
    let events = ListAuditEvents::execute(&repo, 50)?;
    let response = events
        .iter()
        .map(audit_event_to_response)
        .collect::<Vec<_>>();
    serde_json::to_string(&response).map_err(|error| ControllerError::Json(error.to_string()))
}

fn audit_event_to_response(event: &AuditEvent) -> AuditEventResponse {
    let (value_kind, value) = audit_value_to_response(&event.value);
    AuditEventResponse {
        category: event.category.as_str().to_owned(),
        action: event.action.clone(),
        actor: event.actor.as_str().to_owned(),
        target: event.target.as_str().to_owned(),
        value_kind: value_kind.to_owned(),
        value,
        occurred_at_ms: system_time_to_millis(event.occurred_at),
    }
}

fn audit_value_to_response(value: &AuditValue) -> (&'static str, String) {
    match value {
        AuditValue::Plain(value) => ("plain", value.clone()),
        AuditValue::SecretRef(_) => ("secret_ref", "secret_ref".to_owned()),
        AuditValue::Redacted => ("redacted", "redacted".to_owned()),
    }
}

fn agent_to_response(agent: &Agent) -> AgentResponse {
    AgentResponse {
        id: agent.id().as_str().to_owned(),
        name: agent.name().as_str().to_owned(),
        status: agent_status_to_str(agent.status()).to_owned(),
        fingerprint: agent.identity().fingerprint.as_str().to_owned(),
        labels: agent
            .labels()
            .iter()
            .map(|label| AgentLabelResponse {
                key: label.key().to_owned(),
                value: label.value().to_owned(),
            })
            .collect(),
        last_seen_at_ms: agent.last_seen_at().map(system_time_to_millis),
    }
}

fn agent_status_to_str(status: AgentStatus) -> &'static str {
    match status {
        AgentStatus::Pending => "pending",
        AgentStatus::Online => "online",
        AgentStatus::Busy => "busy",
        AgentStatus::Degraded => "degraded",
        AgentStatus::Offline => "offline",
        AgentStatus::Disabled => "disabled",
    }
}

fn parse_drift_status(value: &str) -> DriftStatus {
    match value {
        "compliant" => DriftStatus::Compliant,
        "drifted" => DriftStatus::Drifted,
        _ => DriftStatus::Unknown,
    }
}

fn drift_status_to_str(status: &DriftStatus) -> &'static str {
    match status {
        DriftStatus::Compliant => "compliant",
        DriftStatus::Drifted => "drifted",
        DriftStatus::Unknown => "unknown",
    }
}

fn map_create_command_job_error(
    error: CreateCommandJobError<
        fleet_store::StoreError,
        fleet_store::StoreError,
        fleet_core::IdentityError,
    >,
) -> CreateCommandJobHttpError {
    match error {
        CreateCommandJobError::Domain(error) => {
            CreateCommandJobHttpError::BadRequest(error.to_string())
        }
        CreateCommandJobError::Agent(error) => {
            CreateCommandJobHttpError::BadRequest(error.to_string())
        }
        CreateCommandJobError::NoTargets => CreateCommandJobHttpError::BadRequest(
            "command job requires at least one target".to_owned(),
        ),
        CreateCommandJobError::Repository(fleet_store::StoreError::ConstraintViolation(
            message,
        )) => CreateCommandJobHttpError::Conflict(message),
        CreateCommandJobError::Repository(error) | CreateCommandJobError::Audit(error) => {
            CreateCommandJobHttpError::Internal(ControllerError::Store(error))
        }
        CreateCommandJobError::Sign(error) => {
            CreateCommandJobHttpError::Internal(ControllerError::Json(error.to_string()))
        }
    }
}

fn map_create_drift_check_job_error(
    error: CreateDriftCheckJobError<
        fleet_store::StoreError,
        fleet_store::StoreError,
        fleet_core::IdentityError,
    >,
) -> CreateCommandJobHttpError {
    match error {
        CreateDriftCheckJobError::Domain(error) => {
            CreateCommandJobHttpError::BadRequest(error.to_string())
        }
        CreateDriftCheckJobError::Agent(error) => {
            CreateCommandJobHttpError::BadRequest(error.to_string())
        }
        CreateDriftCheckJobError::NoTargets => CreateCommandJobHttpError::BadRequest(
            "drift check job requires at least one target".to_owned(),
        ),
        CreateDriftCheckJobError::Repository(fleet_store::StoreError::ConstraintViolation(
            message,
        )) => CreateCommandJobHttpError::Conflict(message),
        CreateDriftCheckJobError::Repository(error) | CreateDriftCheckJobError::Audit(error) => {
            CreateCommandJobHttpError::Internal(ControllerError::Store(error))
        }
        CreateDriftCheckJobError::Sign(error) => {
            CreateCommandJobHttpError::Internal(ControllerError::Json(error.to_string()))
        }
    }
}

fn map_create_runbook_job_error(
    error: CreateRunbookJobError<
        fleet_store::StoreError,
        fleet_store::StoreError,
        fleet_core::IdentityError,
    >,
) -> CreateCommandJobHttpError {
    match error {
        CreateRunbookJobError::Domain(error) => {
            CreateCommandJobHttpError::BadRequest(error.to_string())
        }
        CreateRunbookJobError::Agent(error) => {
            CreateCommandJobHttpError::BadRequest(error.to_string())
        }
        CreateRunbookJobError::InvalidRunbook(error) => {
            CreateCommandJobHttpError::BadRequest(error)
        }
        CreateRunbookJobError::NoTargets => CreateCommandJobHttpError::BadRequest(
            "runbook job requires at least one target".to_owned(),
        ),
        CreateRunbookJobError::Repository(fleet_store::StoreError::ConstraintViolation(
            message,
        )) => CreateCommandJobHttpError::Conflict(message),
        CreateRunbookJobError::Repository(error) | CreateRunbookJobError::Audit(error) => {
            CreateCommandJobHttpError::Internal(ControllerError::Store(error))
        }
        CreateRunbookJobError::Sign(error) => {
            CreateCommandJobHttpError::Internal(ControllerError::Json(error.to_string()))
        }
    }
}

fn map_update_agent_labels_error(
    error: UpdateAgentLabelsError<fleet_store::StoreError, fleet_store::StoreError>,
) -> ControllerError {
    match error {
        UpdateAgentLabelsError::Agent(error) => ControllerError::Store(error.into()),
        UpdateAgentLabelsError::Repository(error) | UpdateAgentLabelsError::Audit(error) => {
            ControllerError::Store(error)
        }
    }
}

struct ControllerAdminTokenRepository<'a> {
    store: &'a SqliteStore,
}

impl AdminTokenRepository for ControllerAdminTokenRepository<'_> {
    type Error = fleet_store::StoreError;

    fn admin_token_exists(&self) -> Result<bool, Self::Error> {
        self.store.admin_token_exists()
    }

    fn insert_admin_token_hash(&mut self, token_hash: &str) -> Result<(), Self::Error> {
        self.store.insert_admin_token_hash(token_hash)
    }

    fn verify_admin_token_hash(&self, token_hash: &str) -> Result<bool, Self::Error> {
        self.store.verify_admin_token_hash(token_hash)
    }
}

struct ControllerAgentInventoryRepository<'a> {
    store: &'a SqliteStore,
}

impl AgentInventoryRepository for ControllerAgentInventoryRepository<'_> {
    type Error = fleet_store::StoreError;

    fn list_agents(&self) -> Result<Vec<Agent>, Self::Error> {
        self.store.list_agents()
    }

    fn find_agent_by_id(&self, id: &AgentId) -> Result<Option<Agent>, Self::Error> {
        self.store.find_agent_by_id(id.as_str())
    }

    fn update_agent_labels(
        &mut self,
        id: &AgentId,
        labels: &[AgentLabel],
    ) -> Result<bool, Self::Error> {
        self.store.update_agent_labels(id.as_str(), labels)
    }
}

struct ControllerFactsRepository<'a> {
    store: &'a SqliteStore,
}

impl FactsRepository for ControllerFactsRepository<'_> {
    type Error = fleet_store::StoreError;

    fn insert_facts_snapshot(
        &mut self,
        agent_id: &str,
        body: &str,
        collected_at: SystemTime,
    ) -> Result<(), Self::Error> {
        self.store
            .insert_facts_snapshot(agent_id, body, collected_at)
    }

    fn latest_facts_snapshot(
        &self,
        agent_id: &str,
    ) -> Result<Option<fleet_application::FactsSnapshotRecord>, Self::Error> {
        Ok(self.store.latest_facts_snapshot(agent_id)?.map(|record| {
            fleet_application::FactsSnapshotRecord {
                agent_id: record.agent_id,
                body: record.body,
                collected_at: record.collected_at,
            }
        }))
    }
}

struct ControllerMetricsRepository<'a> {
    store: &'a SqliteStore,
}

impl MetricsRepository for ControllerMetricsRepository<'_> {
    type Error = fleet_store::StoreError;

    fn insert_metrics_snapshot(
        &mut self,
        agent_id: &str,
        body: &str,
        collected_at: SystemTime,
    ) -> Result<(), Self::Error> {
        self.store
            .insert_metrics_snapshot(agent_id, body, collected_at)
    }

    fn latest_metrics_snapshot(
        &self,
        agent_id: &str,
    ) -> Result<Option<fleet_application::MetricsSnapshotRecord>, Self::Error> {
        Ok(self.store.latest_metrics_snapshot(agent_id)?.map(|record| {
            fleet_application::MetricsSnapshotRecord {
                agent_id: record.agent_id,
                body: record.body,
                collected_at: record.collected_at,
            }
        }))
    }
}

struct ControllerDriftRepository<'a> {
    store: &'a SqliteStore,
}

impl DriftRepository for ControllerDriftRepository<'_> {
    type Error = fleet_store::StoreError;

    fn insert_drift_report(
        &mut self,
        agent_id: &str,
        report: &DriftReport,
        checked_at: SystemTime,
    ) -> Result<(), Self::Error> {
        self.store.insert_drift_report(agent_id, report, checked_at)
    }

    fn latest_drift_report(
        &self,
        agent_id: &str,
    ) -> Result<Option<fleet_application::DriftReportRecord>, Self::Error> {
        Ok(self.store.latest_drift_report(agent_id)?.map(|record| {
            fleet_application::DriftReportRecord {
                agent_id: record.agent_id,
                report: record.report,
                checked_at: record.checked_at,
            }
        }))
    }
}

struct ControllerAuditRepository<'a> {
    store: &'a SqliteStore,
}

impl fleet_application::AuditWriter for ControllerAuditRepository<'_> {
    type Error = fleet_store::StoreError;

    fn write(&mut self, event: AuditEvent) -> Result<(), Self::Error> {
        self.store.write_audit_event(event)
    }
}

impl fleet_application::AuditRepository for ControllerAuditRepository<'_> {
    fn list(&self, limit: usize) -> Result<Vec<AuditEvent>, Self::Error> {
        self.store.list_audit_events(limit)
    }

    fn list_by_category(
        &self,
        category: AuditCategory,
        limit: usize,
    ) -> Result<Vec<AuditEvent>, Self::Error> {
        self.store.list_audit_events_by_category(category, limit)
    }
}

struct ControllerEnrollmentTokenRepository<'a> {
    store: &'a SqliteStore,
}

impl EnrollmentTokenRepository for ControllerEnrollmentTokenRepository<'_> {
    type Error = fleet_store::StoreError;

    fn insert_enrollment_token_hash(
        &mut self,
        id: &str,
        token_hash: &str,
        default_labels: &str,
        expires_at: SystemTime,
        max_uses: u32,
    ) -> Result<(), Self::Error> {
        self.store.insert_enrollment_token_hash(
            id,
            token_hash,
            default_labels,
            expires_at,
            max_uses,
        )
    }

    fn list_enrollment_tokens(
        &self,
    ) -> Result<Vec<fleet_application::EnrollmentTokenRecord>, Self::Error> {
        Ok(self
            .store
            .list_enrollment_tokens()?
            .into_iter()
            .map(|record| fleet_application::EnrollmentTokenRecord {
                id: record.id,
                default_labels: record.default_labels,
                expires_at: record.expires_at,
                max_uses: record.max_uses,
                used_count: record.used_count,
                revoked: record.revoked,
            })
            .collect())
    }

    fn revoke_enrollment_token(&mut self, id: &str) -> Result<bool, Self::Error> {
        self.store.revoke_enrollment_token(id)
    }

    fn consume_enrollment_token_hash(
        &mut self,
        token_hash: &str,
        now: SystemTime,
    ) -> Result<fleet_application::EnrollmentTokenRecord, Self::Error> {
        let record = self.store.consume_enrollment_token_hash(token_hash, now)?;
        Ok(fleet_application::EnrollmentTokenRecord {
            id: record.id,
            default_labels: record.default_labels,
            expires_at: record.expires_at,
            max_uses: record.max_uses,
            used_count: record.used_count,
            revoked: record.revoked,
        })
    }
}

struct ControllerJobRepository<'a> {
    store: &'a SqliteStore,
}

struct ControllerJobOutputRepository<'a> {
    store: &'a SqliteStore,
}

impl JobOutputRepository for ControllerJobOutputRepository<'_> {
    type Error = fleet_store::StoreError;

    fn append_output_chunk(&mut self, chunk: JobOutputChunk) -> Result<(), Self::Error> {
        self.store.append_job_output_chunk_record(&chunk)
    }

    fn list_output_chunks(
        &self,
        job_id: &str,
        agent_id: &str,
    ) -> Result<Vec<JobOutputChunk>, Self::Error> {
        self.store.list_job_output_chunks(job_id, agent_id)
    }

    fn list_output_chunks_for_job(&self, job_id: &str) -> Result<Vec<JobOutputChunk>, Self::Error> {
        self.store.list_job_output_chunks_for_job(job_id)
    }
}

struct ControllerJobQueryRepository<'a> {
    store: &'a SqliteStore,
}

impl JobQueryRepository for ControllerJobQueryRepository<'_> {
    type Error = fleet_store::StoreError;

    fn list_job_summaries(
        &self,
        limit: usize,
    ) -> Result<Vec<fleet_application::JobSummaryRecord>, Self::Error> {
        Ok(self
            .store
            .list_job_summaries(limit)?
            .into_iter()
            .map(|record| fleet_application::JobSummaryRecord {
                id: record.id,
                status: record.status,
                risk: record.risk,
                command_program: record.command_program,
                command_args: record.command_args,
                target_count: record.target_count,
                created_at: record.created_at,
            })
            .collect())
    }
}

impl JobRepository for ControllerJobRepository<'_> {
    type Error = fleet_store::StoreError;

    fn save(&mut self, job: Job) -> Result<(), Self::Error> {
        self.store.save_job_record(&job)
    }
}

impl TaskAssignmentRepository for ControllerJobRepository<'_> {
    type Error = fleet_store::StoreError;

    fn save_assignment(&mut self, envelope: TaskEnvelope) -> Result<(), Self::Error> {
        self.store.save_task_assignment_record(&envelope)
    }
}

impl CommandJobRepository for ControllerJobRepository<'_> {
    fn save_command_job(
        &mut self,
        job: Job,
        task: &fleet_domain::CommandTask,
    ) -> Result<(), Self::Error> {
        self.store.save_command_job_record(&job, task)
    }
}

impl fleet_application::DriftCheckJobRepository for ControllerJobRepository<'_> {
    fn save_drift_check_job(
        &mut self,
        job: Job,
        task: &fleet_domain::DriftCheckTask,
    ) -> Result<(), Self::Error> {
        self.store.save_drift_check_job_record(&job, task)
    }
}

impl RunbookJobRepository for ControllerJobRepository<'_> {
    fn save_runbook_job(
        &mut self,
        job: Job,
        task: &fleet_domain::RunbookExecutionTask,
    ) -> Result<(), Self::Error> {
        self.store.save_runbook_job_record(&job, task)
    }
}

struct ControllerAuditWriter<'a> {
    store: &'a SqliteStore,
}

impl fleet_application::AuditWriter for ControllerAuditWriter<'_> {
    type Error = fleet_store::StoreError;

    fn write(&mut self, event: AuditEvent) -> Result<(), Self::Error> {
        self.store.write_audit_event(event)
    }
}

struct ControllerTaskSigner<'a> {
    private_key: &'a str,
}

impl TaskEnvelopeSigner for ControllerTaskSigner<'_> {
    type Error = fleet_core::IdentityError;

    fn sign(&mut self, payload: &str) -> Result<String, Self::Error> {
        fleet_core::sign_challenge(self.private_key, payload)
    }
}

fn enroll_agent(
    body: &str,
    store: &SqliteStore,
    identity: &ControllerIdentity,
) -> Result<String, ControllerError> {
    let request: EnrollAgentRequest =
        serde_json::from_str(body).map_err(|error| ControllerError::Json(error.to_string()))?;
    let expected_fingerprint = fleet_core::fingerprint_public_key(&request.public_key)
        .map_err(|error| ControllerError::Json(error.to_string()))?;
    if expected_fingerprint != request.fingerprint {
        return Err(ControllerError::Store(fleet_store::StoreError::Domain(
            "agent fingerprint does not match public key".to_owned(),
        )));
    }
    let enrollment_token =
        store.consume_enrollment_token_hash(&hash_token(&request.token), SystemTime::now())?;
    let agent_id = request.agent_id.clone();

    let mut labels = parse_label_string(&enrollment_token.default_labels)?;
    for label in request.labels {
        let label = AgentLabel::new(label.key, label.value)
            .map_err(|error| ControllerError::Store(error.into()))?;
        labels.retain(|existing| existing.key() != label.key());
        labels.push(label);
    }
    let mut agent = Agent::new(
        AgentId::new(request.agent_id).map_err(|error| ControllerError::Store(error.into()))?,
        AgentName::new(request.name).map_err(|error| ControllerError::Store(error.into()))?,
        AgentIdentity {
            public_key: AgentPublicKey::new(request.public_key)
                .map_err(|error| ControllerError::Store(error.into()))?,
            fingerprint: AgentFingerprint::new(request.fingerprint)
                .map_err(|error| ControllerError::Store(error.into()))?,
        },
    );
    agent.set_labels(labels);
    agent.pin_controller(
        ControllerPublicKey::new(identity.public_key.clone())
            .map_err(|error| ControllerError::Store(error.into()))?,
    );

    store.save_agent(agent)?;

    serde_json::to_string(&EnrollAgentResponse {
        agent_id,
        controller_public_key: identity.public_key.clone(),
        controller_fingerprint: identity.fingerprint.clone(),
    })
    .map_err(|error| ControllerError::Json(error.to_string()))
}

fn parse_label_string(labels: &str) -> Result<Vec<AgentLabel>, ControllerError> {
    labels
        .split(',')
        .filter(|part| !part.trim().is_empty())
        .map(|part| {
            let (key, value) = part.split_once('=').ok_or_else(|| {
                ControllerError::Store(fleet_store::StoreError::Domain(format!(
                    "invalid default label, expected key=value: {part}"
                )))
            })?;
            AgentLabel::new(key, value).map_err(|error| ControllerError::Store(error.into()))
        })
        .collect()
}

fn request_body(request: &str) -> &str {
    request.split("\r\n\r\n").nth(1).unwrap_or_default()
}

fn authorized(request: &str, store: &SqliteStore) -> Result<bool, ControllerError> {
    let Some(token) = bearer_token(request) else {
        return Ok(false);
    };
    let repo = ControllerAdminTokenRepository { store };
    VerifyAdminToken::execute(&repo, &hash_token(token)).map_err(ControllerError::from)
}

fn bearer_token(request: &str) -> Option<&str> {
    request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("authorization") {
            value.trim().strip_prefix("Bearer ")
        } else {
            None
        }
    })
}

fn response(status: u16, content_type: &str, body: &str) -> String {
    let reason = match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        409 => "Conflict",
        _ => "Internal Server Error",
    };
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn validate_transport(config: &ControllerServerConfig) -> Result<(), ControllerError> {
    if config.dev_insecure_loopback && !is_loopback_host(&config.host) {
        return Err(ControllerError::InsecureRemote(config.host.clone()));
    }
    Ok(())
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn generate_token(prefix: &str) -> Result<String, ControllerError> {
    fleet_core::generate_prefixed_ulid(prefix)
        .map_err(|error| ControllerError::Json(error.to_string()))
}

pub fn heartbeat_signature(nonce: &str, fingerprint: &str) -> String {
    hash_token(&format!("{nonce}:{fingerprint}"))
}

fn default_job_expiration_seconds() -> u64 {
    300
}

fn default_drift_timeout_seconds() -> u64 {
    30
}

fn default_confirmed_by() -> String {
    "admin".to_owned()
}

fn system_time_to_millis(value: SystemTime) -> u64 {
    value
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn epoch_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use fleet_store::SqliteStore;
    use std::io::{Read, Write};

    #[test]
    fn health_endpoint_is_public() {
        let store = SqliteStore::in_memory().unwrap();
        let response = route_request("GET /healthz HTTP/1.1\r\n\r\n", &store).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("\"status\":\"ok\""));
    }

    #[test]
    fn controller_identity_endpoint_is_public() {
        let store = SqliteStore::in_memory().unwrap();
        let response =
            route_request("GET /api/controller/identity HTTP/1.1\r\n\r\n", &store).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("\"controller_fingerprint\":\"dev-controller-fingerprint\""));
    }

    #[test]
    fn admin_static_assets_are_served_by_controller() {
        let store = SqliteStore::in_memory().unwrap();

        let index = route_request("GET /admin HTTP/1.1\r\n\r\n", &store).unwrap();
        let index_with_query =
            route_request("GET /admin?admin-token=redacted HTTP/1.1\r\n\r\n", &store).unwrap();
        let css = route_request("GET /admin/styles.css HTTP/1.1\r\n\r\n", &store).unwrap();
        let js = route_request("GET /admin/app.js HTTP/1.1\r\n\r\n", &store).unwrap();
        let js_with_query =
            route_request("GET /admin/app.js?v=1 HTTP/1.1\r\n\r\n", &store).unwrap();
        let client = route_request("GET /admin/api-client.js HTTP/1.1\r\n\r\n", &store).unwrap();
        let schema = route_request("GET /admin/api.schema.json HTTP/1.1\r\n\r\n", &store).unwrap();
        let missing = route_request("GET /admin/missing.js HTTP/1.1\r\n\r\n", &store).unwrap();

        assert!(index.starts_with("HTTP/1.1 200"));
        assert!(index.contains("Content-Type: text/html; charset=utf-8"));
        assert!(index.contains("Sponzey Fleet Admin"));
        assert!(index.contains("/admin/app.js"));
        assert!(index_with_query.starts_with("HTTP/1.1 200"));
        assert!(index_with_query.contains("Sponzey Fleet Admin"));
        assert!(css.starts_with("HTTP/1.1 200"));
        assert!(css.contains("Content-Type: text/css; charset=utf-8"));
        assert!(css.contains("color-scheme"));
        assert!(js.starts_with("HTTP/1.1 200"));
        assert!(js.contains("Content-Type: application/javascript; charset=utf-8"));
        assert!(js.contains("./api-client.js"));
        assert!(js_with_query.starts_with("HTTP/1.1 200"));
        assert!(js_with_query.contains("createApiClient"));
        assert!(client.starts_with("HTTP/1.1 200"));
        assert!(client.contains("/api/agents"));
        assert!(schema.starts_with("HTTP/1.1 200"));
        assert!(schema.contains("\"schema_version\": \"mvp-1\""));
        assert!(missing.starts_with("HTTP/1.1 404"));
    }

    #[test]
    fn protected_api_requires_admin_token() {
        let store = SqliteStore::in_memory().unwrap();
        let response =
            route_request("POST /api/enrollment-tokens HTTP/1.1\r\n\r\n", &store).unwrap();
        assert!(response.starts_with("HTTP/1.1 401"));
    }

    #[test]
    fn admin_token_can_create_enrollment_token() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();

        let response = route_request(
            "POST /api/enrollment-tokens HTTP/1.1\r\nAuthorization: Bearer admin-token\r\n\r\n",
            &store,
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 201"));
        assert!(response.contains("\"token\":\"enroll-"));
        assert_eq!(store.list_enrollment_tokens().unwrap().len(), 1);
    }

    #[test]
    fn enrollment_token_create_is_audited_without_raw_token() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();

        let response = route_request(
            "POST /api/enrollment-tokens HTTP/1.1\r\nAuthorization: Bearer admin-token\r\n\r\n",
            &store,
        )
        .unwrap();
        let audits = store
            .list_audit_events_by_category(AuditCategory::Enrollment, 10)
            .unwrap();

        assert!(response.starts_with("HTTP/1.1 201"));
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].action, "enrollment_token_created");
        assert!(!audits[0].contains_secret_plaintext());
    }

    #[test]
    fn enrollment_token_revoke_is_audited() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        store
            .insert_enrollment_token_hash(
                "et-1",
                &hash_token("enroll-token"),
                "",
                SystemTime::now() + Duration::from_secs(60),
                1,
            )
            .unwrap();

        let response = route_request(
            "DELETE /api/enrollment-tokens/et-1 HTTP/1.1\r\nAuthorization: Bearer admin-token\r\n\r\n",
            &store,
        )
        .unwrap();
        let audits = store
            .list_audit_events_by_category(AuditCategory::Enrollment, 10)
            .unwrap();

        assert!(response.starts_with("HTTP/1.1 204"));
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].action, "enrollment_token_revoked");
    }

    #[test]
    fn admin_can_create_command_job_with_signed_assignment() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        save_test_agent(&store, "agent-1");
        let body = serde_json::to_string(&CreateCommandJobRequest {
            job_id: "job-1".to_owned(),
            target_agent_ids: vec!["agent-1".to_owned()],
            selector: None,
            program: "uptime".to_owned(),
            args: Vec::new(),
            timeout_seconds: 30,
            confirmed_high_risk: true,
            confirmed_by: "operator-1".to_owned(),
            expires_in_seconds: 60,
            nonce_prefix: Some("nonce".to_owned()),
        })
        .unwrap();
        let request = format!(
            "POST /api/jobs/command HTTP/1.1\r\nAuthorization: Bearer admin-token\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let response = route_request(&request, &store).unwrap();
        let audits = store
            .list_audit_events_by_category(AuditCategory::Job, 10)
            .unwrap();

        assert!(response.starts_with("HTTP/1.1 201"));
        assert!(response.contains("\"target_count\":1"));
        assert!(response.contains("\"assignment_count\":1"));
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].action, "job_created");
        assert_eq!(audits[0].actor.as_str(), "operator-1");
        assert_eq!(
            audits[0].value,
            AuditValue::Plain(
                "confirmed_high_risk=true,confirmed_by=operator-1,target_count=1".to_owned()
            )
        );
    }

    #[test]
    fn admin_can_create_runbook_job_with_signed_assignment() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        save_test_agent(&store, "agent-1");
        let body = serde_json::to_string(&CreateRunbookJobRequest {
            job_id: "job-runbook-1".to_owned(),
            target_agent_ids: vec!["agent-1".to_owned()],
            selector: None,
            runbook_document: r#"
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
"#
            .to_owned(),
            timeout_seconds: 30,
            confirmed_high_risk: true,
            confirmed_by: "operator-1".to_owned(),
            expires_in_seconds: 60,
            nonce_prefix: Some("nonce-runbook".to_owned()),
        })
        .unwrap();
        let request = format!(
            "POST /api/jobs/runbook HTTP/1.1\r\nAuthorization: Bearer admin-token\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let response = route_request(&request, &store).unwrap();
        let assignments = store
            .list_pending_runbook_assignments_for_agent("agent-1")
            .unwrap();
        let audits = store
            .list_audit_events_by_category(AuditCategory::Job, 10)
            .unwrap();

        assert!(response.starts_with("HTTP/1.1 201"));
        assert!(response.contains("\"target_count\":1"));
        assert_eq!(assignments.len(), 1);
        assert!(
            assignments[0]
                .runbook
                .runbook_document()
                .contains("kind: Runbook")
        );
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].action, "runbook_job_created");
    }

    #[test]
    fn command_job_can_target_agents_by_selector() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        save_test_agent_with_labels(&store, "agent-1", vec![("role", "web")]);
        let body = serde_json::to_string(&CreateCommandJobRequest {
            job_id: "job-1".to_owned(),
            target_agent_ids: Vec::new(),
            selector: Some("role=web".to_owned()),
            program: "uptime".to_owned(),
            args: Vec::new(),
            timeout_seconds: 30,
            confirmed_high_risk: true,
            confirmed_by: "operator-1".to_owned(),
            expires_in_seconds: 60,
            nonce_prefix: Some("nonce".to_owned()),
        })
        .unwrap();
        let request = format!(
            "POST /api/jobs/command HTTP/1.1\r\nAuthorization: Bearer admin-token\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let response = route_request(&request, &store).unwrap();

        assert!(response.starts_with("HTTP/1.1 201"));
        assert!(response.contains("\"target_count\":1"));
        assert_eq!(
            store
                .list_pending_command_assignments_for_agent("agent-1")
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn admin_can_create_drift_check_job_with_signed_assignment() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        save_test_agent_with_labels(&store, "agent-1", vec![("role", "web")]);
        let policy_document = r#"
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
        let body = serde_json::to_string(&CreateDriftCheckJobRequest {
            job_id: "drift-job-1".to_owned(),
            target_agent_ids: Vec::new(),
            selector: Some("role=web".to_owned()),
            policy_document: policy_document.to_owned(),
            timeout_seconds: 30,
            created_by: "operator-1".to_owned(),
            expires_in_seconds: 60,
            nonce_prefix: Some("nonce-drift".to_owned()),
        })
        .unwrap();
        let request = format!(
            "POST /api/jobs/drift-check HTTP/1.1\r\nAuthorization: Bearer admin-token\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let response = route_request(&request, &store).unwrap();
        let assignments = store
            .list_pending_drift_check_assignments_for_agent("agent-1")
            .unwrap();
        let audits = store
            .list_audit_events_by_category(AuditCategory::Drift, 10)
            .unwrap();

        assert!(response.starts_with("HTTP/1.1 201"));
        assert!(response.contains("\"target_count\":1"));
        assert!(response.contains("\"assignment_count\":1"));
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].envelope.job_id.as_str(), "drift-job-1");
        assert!(
            assignments[0]
                .drift_check
                .policy_document()
                .contains("nginx-running")
        );
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].action, "drift_check_job_created");
        assert_eq!(audits[0].actor.as_str(), "operator-1");
    }

    #[test]
    fn command_job_selector_excludes_disabled_agents() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        save_test_agent_with_labels(&store, "agent-enabled", vec![("role", "web")]);
        save_disabled_test_agent_with_labels(&store, "agent-disabled", vec![("role", "web")]);
        let body = serde_json::to_string(&CreateCommandJobRequest {
            job_id: "job-1".to_owned(),
            target_agent_ids: Vec::new(),
            selector: Some("role=web".to_owned()),
            program: "uptime".to_owned(),
            args: Vec::new(),
            timeout_seconds: 30,
            confirmed_high_risk: true,
            confirmed_by: "operator-1".to_owned(),
            expires_in_seconds: 60,
            nonce_prefix: Some("nonce".to_owned()),
        })
        .unwrap();
        let request = format!(
            "POST /api/jobs/command HTTP/1.1\r\nAuthorization: Bearer admin-token\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let response = route_request(&request, &store).unwrap();

        assert!(response.starts_with("HTTP/1.1 201"));
        assert!(response.contains("\"target_count\":1"));
        assert_eq!(
            store
                .list_pending_command_assignments_for_agent("agent-enabled")
                .unwrap()
                .len(),
            1
        );
        assert!(
            store
                .list_pending_command_assignments_for_agent("agent-disabled")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn command_job_requires_high_risk_confirmation() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        save_test_agent(&store, "agent-1");
        let body = serde_json::to_string(&CreateCommandJobRequest {
            job_id: "job-1".to_owned(),
            target_agent_ids: vec!["agent-1".to_owned()],
            selector: None,
            program: "uptime".to_owned(),
            args: Vec::new(),
            timeout_seconds: 30,
            confirmed_high_risk: false,
            confirmed_by: "operator-1".to_owned(),
            expires_in_seconds: 60,
            nonce_prefix: Some("nonce".to_owned()),
        })
        .unwrap();
        let request = format!(
            "POST /api/jobs/command HTTP/1.1\r\nAuthorization: Bearer admin-token\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let response = route_request(&request, &store).unwrap();

        assert!(response.starts_with("HTTP/1.1 400"));
        assert!(response.contains("high-risk task requires approval"));
    }

    #[test]
    fn task_output_chunk_is_stored_as_job_output() {
        let store = SqliteStore::in_memory().unwrap();
        save_test_agent(&store, "agent-1");
        save_test_job(&store, "job-1");
        let message = fleet_protocol::WireMessage::new(
            "msg-output",
            "corr-output",
            Some("agent-1".to_owned()),
            1,
            fleet_protocol::WirePayload::OutputChunk {
                job_id: "job-1".to_owned(),
                task_id: "task-1".to_owned(),
                stream: fleet_protocol::OutputStream::Stdout,
                sequence: 0,
                data: "ok".to_owned(),
            },
        );

        let finished = handle_agent_task_data_message(&store, "agent-1", message).unwrap();
        let chunks = store.list_job_output_chunks("job-1", "agent-1").unwrap();

        assert!(!finished);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].body, "ok");
    }

    #[test]
    fn admin_can_poll_job_output_chunks() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        save_test_agent(&store, "agent-1");
        save_test_job(&store, "job-1");
        store
            .append_job_output_chunk_record(&JobOutputChunk {
                job_id: "job-1".to_owned(),
                agent_id: "agent-1".to_owned(),
                stream: JobOutputStream::Stdout,
                sequence: 0,
                body: "ok".to_owned(),
            })
            .unwrap();

        let response = route_request(
            "GET /api/jobs/job-1/output HTTP/1.1\r\nAuthorization: Bearer admin-token\r\n\r\n",
            &store,
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("\"job_id\":\"job-1\""));
        assert!(response.contains("\"agent_id\":\"agent-1\""));
        assert!(response.contains("\"stream\":\"stdout\""));
        assert!(response.contains("\"data\":\"ok\""));
    }

    #[test]
    fn admin_can_list_agents() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        save_test_agent(&store, "agent-1");

        let response = route_request(
            "GET /api/agents HTTP/1.1\r\nAuthorization: Bearer admin-token\r\n\r\n",
            &store,
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("\"id\":\"agent-1\""));
        assert!(response.contains("\"name\":\"agent-1\""));
        assert!(response.contains("\"status\":\"pending\""));
        assert!(response.contains("\"fingerprint\""));
    }

    #[test]
    fn admin_can_get_agent_detail() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        save_test_agent(&store, "agent-1");

        let response = route_request(
            "GET /api/agents/agent-1 HTTP/1.1\r\nAuthorization: Bearer admin-token\r\n\r\n",
            &store,
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("\"id\":\"agent-1\""));
        assert!(response.contains("\"labels\""));
    }

    #[test]
    fn missing_agent_detail_is_not_found() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();

        let response = route_request(
            "GET /api/agents/missing HTTP/1.1\r\nAuthorization: Bearer admin-token\r\n\r\n",
            &store,
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 404"));
    }

    #[test]
    fn admin_can_update_agent_labels_and_audit() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        save_test_agent(&store, "agent-1");
        let body = serde_json::to_string(&UpdateAgentLabelsRequest {
            labels: vec![
                AgentLabelResponse {
                    key: "role".to_owned(),
                    value: "api".to_owned(),
                },
                AgentLabelResponse {
                    key: "env".to_owned(),
                    value: "prod".to_owned(),
                },
            ],
        })
        .unwrap();
        let request = format!(
            "PATCH /api/agents/agent-1/labels HTTP/1.1\r\nAuthorization: Bearer admin-token\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let response = route_request(&request, &store).unwrap();
        let audits = store
            .list_audit_events_by_category(AuditCategory::Agent, 10)
            .unwrap();

        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("\"key\":\"role\""));
        assert!(response.contains("\"value\":\"api\""));
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].action, "agent_labels_updated");
        assert_eq!(
            audits[0].value,
            AuditValue::Plain("label_count=2".to_owned())
        );
    }

    #[test]
    fn invalid_agent_label_update_is_rejected() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        save_test_agent(&store, "agent-1");
        let body = serde_json::to_string(&UpdateAgentLabelsRequest {
            labels: vec![AgentLabelResponse {
                key: "role!".to_owned(),
                value: "api".to_owned(),
            }],
        })
        .unwrap();
        let request = format!(
            "PATCH /api/agents/agent-1/labels HTTP/1.1\r\nAuthorization: Bearer admin-token\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let response = route_request(&request, &store).unwrap();

        assert!(response.starts_with("HTTP/1.1 400"));
        assert!(response.contains("invalid agent label"));
    }

    #[test]
    fn unauthorized_agent_label_update_is_rejected() {
        let store = SqliteStore::in_memory().unwrap();
        save_test_agent(&store, "agent-1");
        let body = serde_json::to_string(&UpdateAgentLabelsRequest {
            labels: vec![AgentLabelResponse {
                key: "role".to_owned(),
                value: "api".to_owned(),
            }],
        })
        .unwrap();
        let request = format!(
            "PATCH /api/agents/agent-1/labels HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let response = route_request(&request, &store).unwrap();

        assert!(response.starts_with("HTTP/1.1 401"));
    }

    #[test]
    fn task_result_updates_job_status_and_audit() {
        let store = SqliteStore::in_memory().unwrap();
        save_test_agent(&store, "agent-1");
        save_test_job(&store, "job-1");
        let message = fleet_protocol::WireMessage::new(
            "msg-result",
            "corr-result",
            Some("agent-1".to_owned()),
            1,
            fleet_protocol::WirePayload::TaskResult {
                job_id: "job-1".to_owned(),
                task_id: "task-1".to_owned(),
                exit_code: 0,
            },
        );

        let finished = handle_agent_task_data_message(&store, "agent-1", message).unwrap();
        let status = store.find_job_status_value("job-1").unwrap().unwrap();
        let audits = store
            .list_audit_events_by_category(AuditCategory::Job, 10)
            .unwrap();

        assert!(finished);
        assert_eq!(status, "success");
        assert_eq!(audits[0].action, "job_completed");
    }

    #[test]
    fn agent_security_event_is_audited() {
        let store = SqliteStore::in_memory().unwrap();
        let message = fleet_protocol::WireMessage::new(
            "msg-security",
            "corr-security",
            Some("agent-1".to_owned()),
            1,
            fleet_protocol::WirePayload::SecurityEvent {
                agent_id: "agent-1".to_owned(),
                action: "task_verification_failed".to_owned(),
                detail: "invalid signature".to_owned(),
            },
        );

        let finished = handle_agent_task_data_message(&store, "agent-1", message).unwrap();
        let audits = store
            .list_audit_events_by_category(AuditCategory::Security, 10)
            .unwrap();

        assert!(finished);
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].action, "task_verification_failed");
        assert!(!audits[0].contains_secret_plaintext());
    }

    #[test]
    fn facts_snapshot_message_is_stored() {
        let store = SqliteStore::in_memory().unwrap();
        save_test_agent(&store, "agent-1");
        let message = fleet_protocol::WireMessage::new(
            "msg-facts",
            "corr-facts",
            Some("agent-1".to_owned()),
            1,
            fleet_protocol::WirePayload::FactsSnapshot {
                agent_id: "agent-1".to_owned(),
                body: "{\"os\":\"linux\",\"arch\":\"x86_64\"}".to_owned(),
            },
        );

        let finished = handle_agent_task_data_message(&store, "agent-1", message).unwrap();
        let snapshot = store.latest_facts_snapshot("agent-1").unwrap().unwrap();

        assert!(!finished);
        assert!(snapshot.body.contains("\"os\":\"linux\""));
    }

    #[test]
    fn degraded_facts_snapshot_marks_agent_degraded() {
        let store = SqliteStore::in_memory().unwrap();
        save_test_agent(&store, "agent-1");
        store
            .mark_agent_online("agent-1", SystemTime::UNIX_EPOCH + Duration::from_secs(1))
            .unwrap();
        let message = fleet_protocol::WireMessage::new(
            "msg-facts",
            "corr-facts",
            Some("agent-1".to_owned()),
            1,
            fleet_protocol::WirePayload::FactsSnapshot {
                agent_id: "agent-1".to_owned(),
                body: "{\"degraded\":{\"status\":true,\"signals\":[\"disk_usage_unavailable\"]}}"
                    .to_owned(),
            },
        );

        handle_agent_task_data_message(&store, "agent-1", message).unwrap();

        let agent = store.find_agent_by_id("agent-1").unwrap().unwrap();
        assert_eq!(agent.status(), AgentStatus::Degraded);
    }

    #[test]
    fn metrics_snapshot_message_is_stored() {
        let store = SqliteStore::in_memory().unwrap();
        save_test_agent(&store, "agent-1");
        let message = fleet_protocol::WireMessage::new(
            "msg-metrics",
            "corr-metrics",
            Some("agent-1".to_owned()),
            1,
            fleet_protocol::WirePayload::MetricsSnapshot {
                agent_id: "agent-1".to_owned(),
                body: "{\"cpu\":{\"logical_count\":4}}".to_owned(),
            },
        );

        let finished = handle_agent_task_data_message(&store, "agent-1", message).unwrap();
        let snapshot = store.latest_metrics_snapshot("agent-1").unwrap().unwrap();

        assert!(!finished);
        assert!(snapshot.body.contains("\"logical_count\":4"));
    }

    #[test]
    fn drift_report_message_is_stored_and_audited() {
        let store = SqliteStore::in_memory().unwrap();
        save_test_agent(&store, "agent-1");
        let message = fleet_protocol::WireMessage::new(
            "msg-drift",
            "corr-drift",
            Some("agent-1".to_owned()),
            1,
            fleet_protocol::WirePayload::DriftReport {
                agent_id: "agent-1".to_owned(),
                status: "drifted".to_owned(),
                expected: "service nginx running".to_owned(),
                actual: "stopped".to_owned(),
            },
        );

        let finished = handle_agent_task_data_message(&store, "agent-1", message).unwrap();
        let record = store.latest_drift_report("agent-1").unwrap().unwrap();
        let audits = store
            .list_audit_events_by_category(AuditCategory::Drift, 10)
            .unwrap();

        assert!(!finished);
        assert_eq!(record.report.status, DriftStatus::Drifted);
        assert_eq!(record.report.actual, "stopped");
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].action, "drift_report_received");
    }

    #[test]
    fn admin_can_get_latest_facts() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        save_test_agent(&store, "agent-1");
        store
            .insert_facts_snapshot(
                "agent-1",
                "{\"os\":\"linux\",\"arch\":\"x86_64\"}",
                SystemTime::UNIX_EPOCH + Duration::from_secs(1),
            )
            .unwrap();

        let response = route_request(
            "GET /api/agents/agent-1/facts/latest HTTP/1.1\r\nAuthorization: Bearer admin-token\r\n\r\n",
            &store,
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("\"agent_id\":\"agent-1\""));
        assert!(response.contains("\"os\":\"linux\""));
    }

    #[test]
    fn admin_can_get_latest_metrics() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        save_test_agent(&store, "agent-1");
        store
            .insert_metrics_snapshot(
                "agent-1",
                "{\"cpu\":{\"logical_count\":4}}",
                SystemTime::UNIX_EPOCH + Duration::from_secs(1),
            )
            .unwrap();

        let response = route_request(
            "GET /api/agents/agent-1/metrics/latest HTTP/1.1\r\nAuthorization: Bearer admin-token\r\n\r\n",
            &store,
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("\"agent_id\":\"agent-1\""));
        assert!(response.contains("\"logical_count\":4"));
    }

    #[test]
    fn admin_can_get_latest_drift_report() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        save_test_agent(&store, "agent-1");
        store
            .insert_drift_report(
                "agent-1",
                &DriftReport {
                    policy_name: "nginx-running".to_owned(),
                    status: DriftStatus::Drifted,
                    expected: "service nginx running".to_owned(),
                    actual: "stopped".to_owned(),
                },
                SystemTime::UNIX_EPOCH + Duration::from_secs(1),
            )
            .unwrap();

        let response = route_request(
            "GET /api/agents/agent-1/drift/latest HTTP/1.1\r\nAuthorization: Bearer admin-token\r\n\r\n",
            &store,
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("\"agent_id\":\"agent-1\""));
        assert!(response.contains("\"status\":\"drifted\""));
        assert!(response.contains("\"actual\":\"stopped\""));
    }

    #[test]
    fn admin_can_list_jobs() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        save_test_agent(&store, "agent-1");
        let body = serde_json::to_string(&CreateCommandJobRequest {
            job_id: "job-history-1".to_owned(),
            target_agent_ids: vec!["agent-1".to_owned()],
            selector: None,
            program: "uptime".to_owned(),
            args: vec!["-a".to_owned()],
            timeout_seconds: 30,
            confirmed_high_risk: true,
            confirmed_by: "admin-token".to_owned(),
            expires_in_seconds: 60,
            nonce_prefix: Some("job-history".to_owned()),
        })
        .unwrap();
        let request = format!(
            "POST /api/jobs/command HTTP/1.1\r\nAuthorization: Bearer admin-token\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        route_request_with_identity(&request, &store, &ControllerIdentity::dev_insecure()).unwrap();

        let response = route_request(
            "GET /api/jobs HTTP/1.1\r\nAuthorization: Bearer admin-token\r\n\r\n",
            &store,
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("\"id\":\"job-history-1\""));
        assert!(response.contains("\"command_program\":\"uptime\""));
        assert!(response.contains("\"target_count\":1"));
    }

    #[test]
    fn admin_can_list_audit_events_without_secret_values() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_admin_token_hash(&hash_token("admin-token"))
            .unwrap();
        store
            .write_audit_event(AuditEvent {
                category: AuditCategory::Security,
                action: "invalid_signature".to_owned(),
                actor: AuditActor::new("system"),
                target: AuditTarget::new("agent-1"),
                value: AuditValue::SecretRef("token=raw-secret".to_owned()),
                occurred_at: SystemTime::UNIX_EPOCH,
            })
            .unwrap();

        let response = route_request(
            "GET /api/audit HTTP/1.1\r\nAuthorization: Bearer admin-token\r\n\r\n",
            &store,
        )
        .unwrap();

        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("\"category\":\"security\""));
        assert!(response.contains("\"action\":\"invalid_signature\""));
        assert!(response.contains("\"value_kind\":\"secret_ref\""));
        assert!(!response.contains("raw-secret"));
    }

    #[test]
    fn task_assignment_wire_includes_command_payload() {
        let envelope = TaskEnvelope {
            job_id: fleet_domain::JobId::new("job-1").unwrap(),
            task_id: fleet_domain::TaskId::new("task-1").unwrap(),
            target_agent_id: AgentId::new("agent-1").unwrap(),
            issued_at: SystemTime::UNIX_EPOCH,
            expires_at: fleet_domain::TaskExpiry::new(
                SystemTime::UNIX_EPOCH + Duration::from_secs(60),
            ),
            nonce: fleet_domain::TaskNonce::new("nonce-1").unwrap(),
            payload_hash: "hash".to_owned(),
            signature: Some(fleet_domain::TaskSignature::new("sig").unwrap()),
        };
        let command =
            fleet_domain::CommandTask::new("uptime", Vec::new(), Duration::from_secs(30)).unwrap();

        let envelope = task_envelope_to_wire(&envelope);
        let task = command_task_to_wire(&command);

        assert_eq!(envelope.job_id, "job-1");
        assert!(matches!(task, fleet_protocol::TaskWire::Command(_)));
    }

    #[test]
    fn agent_enroll_consumes_token_and_registers_agent() {
        let store = SqliteStore::in_memory().unwrap();
        let key_pair = fleet_core::generate_agent_key_pair().unwrap();
        store
            .insert_enrollment_token_hash(
                "et-1",
                &hash_token("enroll-token"),
                "role=web",
                SystemTime::now() + Duration::from_secs(60),
                1,
            )
            .unwrap();

        let body = serde_json::to_string(&EnrollAgentRequest {
            token: "enroll-token".to_owned(),
            agent_id: "agent-web-01".to_owned(),
            name: "web-01".to_owned(),
            public_key: key_pair.public_key_hex,
            fingerprint: key_pair.fingerprint,
            labels: vec![EnrollAgentLabel {
                key: "role".to_owned(),
                value: "web".to_owned(),
            }],
        })
        .unwrap();
        let request = format!(
            "POST /api/agents/enroll HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let response = route_request(&request, &store).unwrap();

        assert!(response.starts_with("HTTP/1.1 201"));
        assert!(response.contains("\"controller_fingerprint\":\"dev-controller-fingerprint\""));
        assert_eq!(store.agent_count().unwrap(), 1);
        assert_eq!(store.list_enrollment_tokens().unwrap()[0].used_count, 1);
    }

    #[test]
    fn agent_enroll_applies_default_labels_from_token() {
        let store = SqliteStore::in_memory().unwrap();
        let key_pair = fleet_core::generate_agent_key_pair().unwrap();
        store
            .insert_enrollment_token_hash(
                "et-1",
                &hash_token("enroll-token"),
                "role=web,env=dev",
                SystemTime::now() + Duration::from_secs(60),
                1,
            )
            .unwrap();

        let body = serde_json::to_string(&EnrollAgentRequest {
            token: "enroll-token".to_owned(),
            agent_id: "agent-web-01".to_owned(),
            name: "web-01".to_owned(),
            public_key: key_pair.public_key_hex,
            fingerprint: key_pair.fingerprint,
            labels: vec![EnrollAgentLabel {
                key: "zone".to_owned(),
                value: "a".to_owned(),
            }],
        })
        .unwrap();
        let request = format!(
            "POST /api/agents/enroll HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let response = route_request(&request, &store).unwrap();
        let labels = store.list_agents().unwrap()[0]
            .labels()
            .iter()
            .map(|label| format!("{}={}", label.key(), label.value()))
            .collect::<Vec<_>>();

        assert!(response.starts_with("HTTP/1.1 201"));
        assert_eq!(labels, ["role=web", "env=dev", "zone=a"]);
    }

    #[test]
    fn explicit_agent_label_overrides_token_default_label() {
        let store = SqliteStore::in_memory().unwrap();
        let key_pair = fleet_core::generate_agent_key_pair().unwrap();
        store
            .insert_enrollment_token_hash(
                "et-1",
                &hash_token("enroll-token"),
                "role=default,env=dev",
                SystemTime::now() + Duration::from_secs(60),
                1,
            )
            .unwrap();

        let body = serde_json::to_string(&EnrollAgentRequest {
            token: "enroll-token".to_owned(),
            agent_id: "agent-web-01".to_owned(),
            name: "web-01".to_owned(),
            public_key: key_pair.public_key_hex,
            fingerprint: key_pair.fingerprint,
            labels: vec![EnrollAgentLabel {
                key: "role".to_owned(),
                value: "web".to_owned(),
            }],
        })
        .unwrap();
        let request = format!(
            "POST /api/agents/enroll HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let response = route_request(&request, &store).unwrap();
        let labels = store.list_agents().unwrap()[0]
            .labels()
            .iter()
            .map(|label| format!("{}={}", label.key(), label.value()))
            .collect::<Vec<_>>();

        assert!(response.starts_with("HTTP/1.1 201"));
        assert_eq!(labels, ["env=dev", "role=web"]);
    }

    #[test]
    fn agent_enroll_rejects_invalid_token() {
        let store = SqliteStore::in_memory().unwrap();
        let key_pair = fleet_core::generate_agent_key_pair().unwrap();
        let body = serde_json::to_string(&EnrollAgentRequest {
            token: "bad-token".to_owned(),
            agent_id: "agent-web-01".to_owned(),
            name: "web-01".to_owned(),
            public_key: key_pair.public_key_hex,
            fingerprint: key_pair.fingerprint,
            labels: Vec::new(),
        })
        .unwrap();
        let request = format!(
            "POST /api/agents/enroll HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let response = route_request(&request, &store).unwrap();

        assert!(response.starts_with("HTTP/1.1 401"));
    }

    #[test]
    fn agent_enroll_rejects_fingerprint_public_key_mismatch() {
        let store = SqliteStore::in_memory().unwrap();
        let key_pair = fleet_core::generate_agent_key_pair().unwrap();
        store
            .insert_enrollment_token_hash(
                "et-1",
                &hash_token("enroll-token"),
                "",
                SystemTime::now() + Duration::from_secs(60),
                1,
            )
            .unwrap();

        let body = serde_json::to_string(&EnrollAgentRequest {
            token: "enroll-token".to_owned(),
            agent_id: "agent-web-01".to_owned(),
            name: "web-01".to_owned(),
            public_key: key_pair.public_key_hex,
            fingerprint: "0123456789abcdef".to_owned(),
            labels: Vec::new(),
        })
        .unwrap();
        let request = format!(
            "POST /api/agents/enroll HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let response = route_request(&request, &store).unwrap();

        assert!(response.starts_with("HTTP/1.1 400"));
        assert!(response.contains("fingerprint does not match public key"));
    }

    #[test]
    fn duplicate_agent_name_is_conflict() {
        let store = SqliteStore::in_memory().unwrap();
        let first = fleet_core::generate_agent_key_pair().unwrap();
        let second = fleet_core::generate_agent_key_pair().unwrap();
        for (id, token, key_pair) in [
            ("agent-web-01", "enroll-token-1", first),
            ("agent-web-02", "enroll-token-2", second),
        ] {
            store
                .insert_enrollment_token_hash(
                    &format!("et-{id}"),
                    &hash_token(token),
                    "",
                    SystemTime::now() + Duration::from_secs(60),
                    1,
                )
                .unwrap();
            let body = serde_json::to_string(&EnrollAgentRequest {
                token: token.to_owned(),
                agent_id: id.to_owned(),
                name: "web-01".to_owned(),
                public_key: key_pair.public_key_hex,
                fingerprint: key_pair.fingerprint,
                labels: Vec::new(),
            })
            .unwrap();
            let request = format!(
                "POST /api/agents/enroll HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let response = route_request(&request, &store).unwrap();
            if id == "agent-web-01" {
                assert!(response.starts_with("HTTP/1.1 201"));
            } else {
                assert!(response.starts_with("HTTP/1.1 409"));
            }
        }
    }

    #[test]
    fn insecure_mode_rejects_remote_host() {
        let config = ControllerServerConfig {
            host: "0.0.0.0".to_owned(),
            port: 7700,
            data_dir: PathBuf::from(".sponzey"),
            database_path: None,
            dev_insecure_loopback: true,
        };
        assert!(matches!(
            validate_transport(&config),
            Err(ControllerError::InsecureRemote(_))
        ));
    }

    #[test]
    fn controller_server_starts_and_stops_on_shutdown_signal() {
        let data_dir = unique_test_dir("controller-shutdown");
        std::fs::create_dir_all(data_dir.join("controller")).unwrap();
        let key_pair = fleet_core::generate_agent_key_pair().unwrap();
        std::fs::write(
            data_dir.join("controller").join("controller_public.key"),
            format!("{}\n", key_pair.public_key_hex),
        )
        .unwrap();
        std::fs::write(
            data_dir.join("controller").join("controller_private.key"),
            format!("{}\n", key_pair.private_key_hex),
        )
        .unwrap();

        let Some(port) = free_loopback_port() else {
            return;
        };
        let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let thread_shutdown = shutdown.clone();
        let handle = std::thread::spawn(move || {
            start_controller_server_until(
                ControllerServerConfig {
                    host: "127.0.0.1".to_owned(),
                    port,
                    data_dir,
                    database_path: None,
                    dev_insecure_loopback: true,
                },
                move || thread_shutdown.load(std::sync::atomic::Ordering::SeqCst),
            )
            .unwrap();
        });

        let response = poll_http_get(port, "/healthz");
        shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
        handle.join().unwrap();

        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("\"status\":\"ok\""));
    }

    #[test]
    fn auth_failure_writes_security_audit() {
        let store = SqliteStore::in_memory().unwrap();

        audit_security(&store, "websocket_invalid_signature", "agent-1").unwrap();

        assert_eq!(
            store
                .audit_count_by_category(fleet_domain::AuditCategory::Security)
                .unwrap(),
            1
        );
    }

    #[test]
    fn dev_insecure_loopback_start_is_audited() {
        let store = SqliteStore::in_memory().unwrap();

        audit_dev_insecure_loopback_enabled(&store, "127.0.0.1").unwrap();

        let audits = store
            .list_audit_events_by_category(AuditCategory::Security, 10)
            .unwrap();
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].action, "dev_insecure_loopback_enabled");
        assert_eq!(audits[0].actor.as_str(), "controller");
        assert_eq!(audits[0].target.as_str(), "127.0.0.1");
        assert_eq!(
            audits[0].value,
            AuditValue::Plain("loopback_only".to_owned())
        );
    }

    #[test]
    fn invalid_agent_signature_is_rejected() {
        let key_pair = fleet_core::generate_agent_key_pair().unwrap();
        let signature = fleet_core::sign_challenge(&key_pair.private_key_hex, "nonce-1").unwrap();

        assert!(!verify_agent_auth_response(
            &key_pair.public_key_hex,
            "nonce-2",
            "nonce-2",
            &signature
        ));
    }

    #[test]
    fn unknown_agent_id_is_rejected_and_audited() {
        let store = SqliteStore::in_memory().unwrap();

        let result = validate_agent_ws_hello(&store, "agent-missing", "fingerprint").unwrap();

        assert!(result.is_none());
        assert_eq!(
            store
                .audit_count_by_category(fleet_domain::AuditCategory::Security)
                .unwrap(),
            1
        );
    }

    #[test]
    fn enrollment_token_cannot_authenticate_websocket_channel() {
        let store = SqliteStore::in_memory().unwrap();
        store
            .insert_enrollment_token_hash(
                "et-1",
                &hash_token("enroll-token"),
                "",
                SystemTime::now() + Duration::from_secs(60),
                1,
            )
            .unwrap();

        let result = validate_agent_ws_hello(&store, "enroll-token", "0123456789abcdef").unwrap();

        assert!(result.is_none());
        assert_eq!(
            store
                .audit_count_by_category(fleet_domain::AuditCategory::Security)
                .unwrap(),
            1
        );
        assert_eq!(store.list_enrollment_tokens().unwrap()[0].used_count, 0);
    }

    #[test]
    fn mismatched_agent_fingerprint_is_rejected_and_audited() {
        let store = SqliteStore::in_memory().unwrap();
        let key_pair = fleet_core::generate_agent_key_pair().unwrap();
        store
            .save_agent(agent_fixture(
                "agent-web-01",
                "web-01",
                &key_pair.public_key_hex,
                &key_pair.fingerprint,
            ))
            .unwrap();

        let result = validate_agent_ws_hello(&store, "agent-web-01", "0123456789abcdef").unwrap();

        assert!(result.is_none());
        assert_eq!(
            store
                .audit_count_by_category(fleet_domain::AuditCategory::Security)
                .unwrap(),
            1
        );
    }

    fn agent_fixture(id: &str, name: &str, public_key: &str, fingerprint: &str) -> Agent {
        Agent::new(
            AgentId::new(id).unwrap(),
            AgentName::new(name).unwrap(),
            AgentIdentity {
                public_key: AgentPublicKey::new(public_key).unwrap(),
                fingerprint: AgentFingerprint::new(fingerprint).unwrap(),
            },
        )
    }

    fn save_test_agent(store: &SqliteStore, id: &str) {
        let key_pair = fleet_core::generate_agent_key_pair().unwrap();
        store
            .save_agent(agent_fixture(
                id,
                id,
                &key_pair.public_key_hex,
                &key_pair.fingerprint,
            ))
            .unwrap();
    }

    fn save_test_agent_with_labels(store: &SqliteStore, id: &str, labels: Vec<(&str, &str)>) {
        let key_pair = fleet_core::generate_agent_key_pair().unwrap();
        let mut agent = agent_fixture(id, id, &key_pair.public_key_hex, &key_pair.fingerprint);
        agent.set_labels(
            labels
                .into_iter()
                .map(|(key, value)| AgentLabel::new(key, value).unwrap())
                .collect(),
        );
        store.save_agent(agent).unwrap();
    }

    fn save_disabled_test_agent_with_labels(
        store: &SqliteStore,
        id: &str,
        labels: Vec<(&str, &str)>,
    ) {
        let key_pair = fleet_core::generate_agent_key_pair().unwrap();
        let mut agent = agent_fixture(id, id, &key_pair.public_key_hex, &key_pair.fingerprint);
        agent.set_labels(
            labels
                .into_iter()
                .map(|(key, value)| AgentLabel::new(key, value).unwrap())
                .collect(),
        );
        agent.disable();
        store.save_agent(agent).unwrap();
    }

    fn save_test_job(store: &SqliteStore, id: &str) {
        let mut job = Job::new(
            fleet_domain::JobId::new(id).unwrap(),
            fleet_domain::TaskRisk::High,
            fleet_domain::ApprovalRequirement::AdminConfirmation,
            Duration::from_secs(30),
        );
        job.queue(true).unwrap();
        store.save_job_record(&job).unwrap();
    }

    fn free_loopback_port() -> Option<u16> {
        std::net::TcpListener::bind("127.0.0.1:0")
            .ok()
            .and_then(|listener| listener.local_addr().ok().map(|addr| addr.port()))
    }

    fn poll_http_get(port: u16, path: &str) -> String {
        let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
        for _ in 0..100 {
            match std::net::TcpStream::connect(("127.0.0.1", port)) {
                Ok(mut stream) => {
                    stream.write_all(request.as_bytes()).unwrap();
                    let mut buffer = [0_u8; 4096];
                    let read = stream.read(&mut buffer).unwrap();
                    return String::from_utf8_lossy(&buffer[..read]).to_string();
                }
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
        panic!("controller did not accept HTTP requests");
    }

    fn unique_test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "sponzey-fleet-controller-{name}-{}-{}",
            std::process::id(),
            epoch_millis()
        ))
    }
}
