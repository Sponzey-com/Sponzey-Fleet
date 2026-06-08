import { createApiClient } from "./api-client.js";

const state = {
  token: "",
  selectedAgentId: "",
  lastJobId: "",
};

const api = createApiClient({
  tokenProvider: () => state.token,
  formatError: formatApiError,
});

export function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

export function renderAgents(agents, selectedAgentId = "") {
  if (!Array.isArray(agents) || agents.length === 0) {
    return '<div class="empty">No agents enrolled.</div>';
  }
  return agents
    .map((agent) => {
      const labels = Array.isArray(agent.labels)
        ? agent.labels.map((label) => `${label.key}=${label.value}`).join(", ")
        : "";
      const selectedClass = agent.id === selectedAgentId ? " selected" : "";
      return `
        <button class="agent-row${selectedClass}" type="button" data-agent-id="${escapeHtml(agent.id)}">
          <span>
            <strong>${escapeHtml(agent.name || agent.id)}</strong>
            <small>${escapeHtml(agent.id)}</small>
          </span>
          <span class="status-pill ${escapeHtml(agent.status || "unknown")}">${escapeHtml(agent.status || "unknown")}</span>
          <small class="labels">${escapeHtml(labels || "no labels")}</small>
        </button>
      `;
    })
    .join("");
}

export function renderSnapshot(snapshot, missingText) {
  if (!snapshot || !snapshot.body) {
    return missingText;
  }
  return JSON.stringify(snapshot.body, null, 2);
}

export function renderDrift(report) {
  if (!report) {
    return '<div class="empty">No drift report.</div>';
  }
  return `
    <div class="drift-summary">
      <span class="status-pill ${escapeHtml(report.status)}">${escapeHtml(report.status)}</span>
      <strong>${escapeHtml(report.policy_name)}</strong>
    </div>
    <div class="diff-grid">
      <section>
        <h3>Expected</h3>
        <pre>${escapeHtml(report.expected)}</pre>
      </section>
      <section>
        <h3>Actual</h3>
        <pre>${escapeHtml(report.actual)}</pre>
      </section>
    </div>
  `;
}

export function renderAudit(events) {
  if (!Array.isArray(events) || events.length === 0) {
    return '<div class="empty">No audit events.</div>';
  }
  return events
    .map(
      (event) => `
        <div class="audit-row">
          <span class="status-pill ${escapeHtml(event.category)}">${escapeHtml(event.category)}</span>
          <strong>${escapeHtml(event.action)}</strong>
          <small>${escapeHtml(event.actor)} -> ${escapeHtml(event.target)}</small>
          <code>${escapeHtml(event.value_kind)}:${escapeHtml(event.value)}</code>
        </div>
      `,
    )
    .join("");
}

export function renderJobs(jobs) {
  if (!Array.isArray(jobs) || jobs.length === 0) {
    return '<div class="empty">No jobs created.</div>';
  }
  return jobs
    .map((job) => {
      const command = [job.command_program, ...(job.command_args || [])].filter(Boolean).join(" ");
      return `
        <button class="job-row" type="button" data-job-id="${escapeHtml(job.id)}">
          <span>
            <strong>${escapeHtml(job.id)}</strong>
            <small>${escapeHtml(command || "non-command job")}</small>
          </span>
          <span class="status-pill ${escapeHtml(job.status || "unknown")}">${escapeHtml(job.status || "unknown")}</span>
          <small>${escapeHtml(job.target_count ?? 0)} target(s)</small>
        </button>
      `;
    })
    .join("");
}

export function parseCommandArgs(value) {
  return String(value ?? "")
    .split(/\s+/)
    .map((part) => part.trim())
    .filter(Boolean);
}

export function buildCommandJobRequest({ agentId, program, args, confirmed }) {
  if (!agentId) {
    throw new Error("Select an agent before running a command.");
  }
  if (!program || !String(program).trim()) {
    throw new Error("Program is required.");
  }
  if (!confirmed) {
    throw new Error("High-risk command confirmation is required.");
  }
  const jobId = `job-ui-${Date.now()}`;
  return {
    job_id: jobId,
    target_agent_ids: [agentId],
    program: String(program).trim(),
    args: Array.isArray(args) ? args : parseCommandArgs(args),
    timeout_seconds: 30,
    confirmed_high_risk: true,
    confirmed_by: "web-admin",
    expires_in_seconds: 60,
    nonce_prefix: jobId,
  };
}

export function renderJobOutput(chunks) {
  if (!Array.isArray(chunks) || chunks.length === 0) {
    return "No job output.";
  }
  return chunks
    .map((chunk) => {
      const prefix = `${chunk.agent_id || "agent"} ${chunk.stream || "stdout"}`;
      return `[${prefix}] ${chunk.data || ""}`;
    })
    .join("");
}

export function formatApiError(path, status) {
  if (status === 401 || status === 403) {
    return "Controller rejected this request. Check the admin token and permissions.";
  }
  return `${path} returned ${status}`;
}

function setStatus(message, kind = "") {
  const element = document.querySelector("#status");
  element.textContent = message;
  element.className = `status ${kind}`.trim();
}

async function loadAgents() {
  const agents = await api.listAgents();
  const selected = state.selectedAgentId || agents[0]?.id || "";
  state.selectedAgentId = selected;
  document.querySelector("#agent-count").textContent = `${agents.length} known`;
  document.querySelector("#agents-list").innerHTML = renderAgents(agents, selected);
  document.querySelectorAll("[data-agent-id]").forEach((button) => {
    button.addEventListener("click", () => {
      state.selectedAgentId = button.dataset.agentId;
      refreshSelectedAgent();
      document.querySelector("#agents-list").innerHTML = renderAgents(agents, state.selectedAgentId);
    });
  });
  if (selected) {
    await refreshSelectedAgent();
  }
}

async function refreshSelectedAgent() {
  const agentId = state.selectedAgentId;
  if (!agentId) {
    return;
  }
  const [facts, metrics, drift] = await Promise.all([
    api.getLatestFacts(agentId),
    api.getLatestMetrics(agentId),
    api.getLatestDrift(agentId),
  ]);
  document.querySelector("#facts-panel").textContent = renderSnapshot(facts, "No facts snapshot.");
  document.querySelector("#metrics-panel").textContent = renderSnapshot(metrics, "No metrics snapshot.");
  document.querySelector("#drift-panel").innerHTML = renderDrift(drift);
}

async function refreshAll() {
  if (!state.token) {
    setStatus("Admin token is required.", "error");
    return;
  }
  setStatus("Loading controller data...");
  await loadAgents();
  const [jobs, audit] = await Promise.all([api.listJobs(), api.listAudit()]);
  document.querySelector("#jobs-list").innerHTML = renderJobs(jobs);
  document.querySelectorAll("[data-job-id]").forEach((button) => {
    button.addEventListener("click", () => {
      state.lastJobId = button.dataset.jobId;
      pollJobOutput(state.lastJobId).catch((error) => setStatus(error.message, "error"));
    });
  });
  document.querySelector("#audit-list").innerHTML = renderAudit(audit);
  setStatus("Loaded latest controller data.", "ok");
}

async function submitCommand(form) {
  const data = new FormData(form);
  const request = buildCommandJobRequest({
    agentId: state.selectedAgentId,
    program: data.get("program"),
    args: data.get("args"),
    confirmed: data.get("confirm-risk") === "on",
  });
  const response = await api.createCommandJob(request);
  state.lastJobId = response.job_id;
  document.querySelector("#job-output").textContent = "Job queued. Waiting for output...";
  setStatus(`Created ${response.job_id} for ${response.target_count} target.`, "ok");
  await pollJobOutput(response.job_id);
}

async function pollJobOutput(jobId) {
  for (let attempt = 0; attempt < 10; attempt += 1) {
    const chunks = await api.getJobOutput(jobId);
    document.querySelector("#job-output").textContent = renderJobOutput(chunks);
    if (Array.isArray(chunks) && chunks.length > 0) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
}

function boot() {
  const form = document.querySelector("#admin-auth");
  if (!form) {
    return;
  }
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    state.token = new FormData(form).get("admin-token")?.toString() || "";
    refreshAll().catch((error) => setStatus(error.message, "error"));
  });
  const runForm = document.querySelector("#run-command-form");
  if (runForm) {
    runForm.addEventListener("submit", (event) => {
      event.preventDefault();
      submitCommand(runForm).catch((error) => setStatus(error.message, "error"));
    });
  }
}

if (typeof document !== "undefined") {
  boot();
}
