import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const indexPath = join(root, "index.html");
const stylesPath = join(root, "styles.css");
const appPath = join(root, "app.js");
const clientPath = join(root, "api-client.js");
const schemaPath = join(root, "api.schema.json");
const tsconfigPath = join(root, "tsconfig.json");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

const index = readFileSync(indexPath, "utf8");
const styles = readFileSync(stylesPath, "utf8");
const app = readFileSync(appPath, "utf8");
const schema = JSON.parse(readFileSync(schemaPath, "utf8"));
const tsconfig = JSON.parse(readFileSync(tsconfigPath, "utf8"));

assert(index.includes("Sponzey Fleet Admin"), "index must name the admin UI");
assert(index.includes("id=\"agents-list\""), "index must expose the agents surface");
assert(index.includes("id=\"facts-panel\""), "index must expose the facts surface");
assert(index.includes("id=\"metrics-panel\""), "index must expose the metrics surface");
assert(index.includes("id=\"drift-panel\""), "index must expose the drift surface");
assert(index.includes("id=\"audit-list\""), "index must expose the audit surface");
assert(index.includes("id=\"run-command-form\""), "index must expose command execution");
assert(index.includes("id=\"job-output\""), "index must expose job output");
assert(index.includes("id=\"jobs-list\""), "index must expose job history");
assert(index.includes("/admin/app.js"), "index must load the dependency-free app script from the admin base path");
assert(index.includes("/admin/styles.css"), "index must load styles from the admin base path");
assert(index.includes('method="post"'), "admin auth form must not leak tokens through a query string fallback");
assert(!index.includes("localStorage"), "UI must not store tokens in localStorage");
assert(!index.includes("runtime config"), "UI must not expose runtime config mutation");
assert(styles.includes(".layout"), "styles must include the admin layout");
assert(app.includes("./api-client.js"), "app must use the shared API client");
assert(tsconfig.compilerOptions.checkJs, "tsconfig must enable JS type checking");
assert(schema.schema_version === "mvp-1", "API schema version must match MVP client");
for (const endpoint of [
  "listAgents",
  "getLatestFacts",
  "getLatestMetrics",
  "getLatestDrift",
  "listJobs",
  "getJobOutput",
  "listAudit",
  "createCommandJob",
  "createDriftCheckJob",
  "createRunbookJob",
]) {
  assert(
    schema.endpoints.some((entry) => entry.name === endpoint),
    `API schema must include ${endpoint}`,
  );
}

const {
  renderAgents,
  renderSnapshot,
  renderDrift,
  renderAudit,
  formatApiError,
  parseCommandArgs,
  buildCommandJobRequest,
  renderJobOutput,
  renderJobs,
} = await import(appPath);
const { API_SCHEMA_VERSION, createApiClient } = await import(clientPath);
assert(API_SCHEMA_VERSION === schema.schema_version, "API client and schema versions must match");

const calls = [];
const client = createApiClient({
  tokenProvider: () => "admin-token",
  fetchImpl: async (path, options) => {
    calls.push({ path, options });
    return {
      ok: true,
      status: 200,
      json: async () => ({ path }),
    };
  },
});
await client.listAgents();
await client.getLatestFacts("agent/1");
await client.createCommandJob({ job_id: "job-1" });
await client.createRunbookJob({ job_id: "job-runbook-1" });
assert(calls[0].path === "/api/agents", "client must call agents endpoint");
assert(
  calls[1].path === "/api/agents/agent%2F1/facts/latest",
  "client must encode agent ids in paths",
);
assert(calls[2].path === "/api/jobs/command", "client must call command job endpoint");
assert(calls[2].options.method === "POST", "client must POST command jobs");
assert(calls[3].path === "/api/jobs/runbook", "client must call runbook job endpoint");
assert(calls[3].options.method === "POST", "client must POST runbook jobs");
assert(
  calls[2].options.headers.Authorization === "Bearer admin-token",
  "client must attach bearer token",
);

const agentsHtml = renderAgents([
  { id: "agent-1", name: "web-01", status: "online", labels: [{ key: "role", value: "web" }] },
]);
assert(agentsHtml.includes("web-01"), "agents renderer must include agent name");
assert(agentsHtml.includes("role=web"), "agents renderer must include labels");

const factsText = renderSnapshot({ body: { os: "linux", disk: { usage_available: true } } }, "");
assert(factsText.includes("\"os\": \"linux\""), "facts renderer must show snapshot JSON");

const driftHtml = renderDrift({
  policy_name: "nginx-running",
  status: "drifted",
  expected: "service nginx running",
  actual: "service nginx stopped",
});
assert(driftHtml.includes("Expected"), "drift renderer must include expected section");
assert(driftHtml.includes("service nginx stopped"), "drift renderer must include actual detail");

const auditHtml = renderAudit([
  { category: "security", action: "invalid_signature", actor: "system", target: "agent-1", value_kind: "redacted", value: "redacted" },
]);
assert(auditHtml.includes("invalid_signature"), "audit renderer must include event action");
const jobsHtml = renderJobs([
  { id: "job-1", status: "success", risk: "high", command_program: "uptime", command_args: ["-a"], target_count: 1 },
]);
assert(jobsHtml.includes("job-1"), "job renderer must include job id");
assert(jobsHtml.includes("uptime -a"), "job renderer must include command summary");
assert(
  formatApiError("/api/agents", 401).includes("admin token"),
  "forbidden renderer must guide operator toward authorization",
);
assert(
  JSON.stringify(parseCommandArgs(" -a  -b ")) === JSON.stringify(["-a", "-b"]),
  "command argument parser must split whitespace",
);
const jobRequest = buildCommandJobRequest({
  agentId: "agent-1",
  program: "uptime",
  args: "-a",
  confirmed: true,
});
assert(jobRequest.confirmed_high_risk, "job request must include high-risk confirmation");
assert(jobRequest.target_agent_ids.includes("agent-1"), "job request must target selected agent");
let confirmationFailed = false;
try {
  buildCommandJobRequest({ agentId: "agent-1", program: "uptime", args: "", confirmed: false });
} catch {
  confirmationFailed = true;
}
assert(confirmationFailed, "run command form must require high-risk confirmation");
const output = renderJobOutput([
  { agent_id: "agent-1", stream: "stdout", data: "ok\n" },
  { agent_id: "agent-1", stream: "stderr", data: "warn\n" },
]);
assert(output.includes("[agent-1 stdout] ok"), "job output renderer must prefix stdout");
assert(output.includes("[agent-1 stderr] warn"), "job output renderer must prefix stderr");

console.log("web-admin smoke tests passed");

if (existsSync(join(root, "dist", "index.html"))) {
  console.log("web-admin dist is present");
}
