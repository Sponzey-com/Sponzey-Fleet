export const API_SCHEMA_VERSION = "mvp-1";

function encodePathValue(value) {
  return encodeURIComponent(String(value ?? ""));
}

function defaultFormatApiError(path, status) {
  if (status === 401 || status === 403) {
    return "Controller rejected this request. Check the admin token and permissions.";
  }
  return `${path} returned ${status}`;
}

export function normalizeAdminToken(value) {
  let token = String(value ?? "").trim();
  if (token.toLowerCase().startsWith("bearer ")) {
    token = token.slice("bearer ".length).trim();
  }
  return token;
}

export function createApiClient({ fetchImpl = globalThis.fetch, tokenProvider = () => "", formatError = defaultFormatApiError } = {}) {
  if (typeof fetchImpl !== "function") {
    throw new Error("fetch implementation is required.");
  }

  async function request(path, options = {}) {
    const token = normalizeAdminToken(tokenProvider());
    const response = await fetchImpl(path, {
      ...options,
      headers: {
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
        Accept: "application/json",
        ...(options.body ? { "Content-Type": "application/json" } : {}),
        ...(options.headers || {}),
      },
    });
    if (response.status === 404) {
      return null;
    }
    if (!response.ok) {
      throw new Error(formatError(path, response.status));
    }
    if (response.status === 204) {
      return null;
    }
    return response.json();
  }

  return {
    listAgents() {
      return request("/api/agents");
    },
    getLatestFacts(agentId) {
      return request(`/api/agents/${encodePathValue(agentId)}/facts/latest`);
    },
    getLatestMetrics(agentId) {
      return request(`/api/agents/${encodePathValue(agentId)}/metrics/latest`);
    },
    getLatestDrift(agentId) {
      return request(`/api/agents/${encodePathValue(agentId)}/drift/latest`);
    },
    revokeAgentKey(agentId, body = {}) {
      return request(`/api/agents/${encodePathValue(agentId)}/revoke-key`, {
        method: "POST",
        body: JSON.stringify(body),
      });
    },
    listJobs() {
      return request("/api/jobs");
    },
    getJobOutput(jobId) {
      return request(`/api/jobs/${encodePathValue(jobId)}/output`);
    },
    listAudit() {
      return request("/api/audit");
    },
    listEnrollmentTokens() {
      return request("/api/enrollment-tokens");
    },
    createEnrollmentToken(body) {
      return request("/api/enrollment-tokens", {
        method: "POST",
        body: JSON.stringify(body),
      });
    },
    revokeEnrollmentToken(id) {
      return request(`/api/enrollment-tokens/${encodePathValue(id)}`, {
        method: "DELETE",
      });
    },
    createCommandJob(body) {
      return request("/api/jobs/command", {
        method: "POST",
        body: JSON.stringify(body),
      });
    },
    createDriftCheckJob(body) {
      return request("/api/jobs/drift-check", {
        method: "POST",
        body: JSON.stringify(body),
      });
    },
    createRunbookJob(body) {
      return request("/api/jobs/runbook", {
        method: "POST",
        body: JSON.stringify(body),
      });
    },
  };
}
