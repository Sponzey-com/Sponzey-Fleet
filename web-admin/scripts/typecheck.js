import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const schema = JSON.parse(readFileSync(join(root, "api.schema.json"), "utf8"));
const tsconfig = JSON.parse(readFileSync(join(root, "tsconfig.json"), "utf8"));
const { API_SCHEMA_VERSION, createApiClient } = await import(join(root, "api-client.js"));

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function pathArguments(path) {
  return [...path.matchAll(/\{([^}]+)\}/g)].map((match) => `${match[1]}-example`);
}

assert(tsconfig.compilerOptions.allowJs === true, "tsconfig must allow JavaScript checking");
assert(tsconfig.compilerOptions.checkJs === true, "tsconfig must enable checkJs");
assert(tsconfig.compilerOptions.noEmit === true, "tsconfig must be noEmit for static export");
assert(API_SCHEMA_VERSION === schema.schema_version, "API schema and client versions must match");
assert(Array.isArray(schema.endpoints), "API schema must define endpoint metadata");

const calls = [];
const client = createApiClient({
  tokenProvider: () => "typecheck-token",
  fetchImpl: async (path, options = {}) => {
    calls.push({ path, options });
    return {
      ok: true,
      status: 200,
      json: async () => ({ ok: true }),
    };
  },
});

for (const endpoint of schema.endpoints) {
  assert(endpoint.name && endpoint.method && endpoint.path, "endpoint metadata must be complete");
  assert(
    typeof client[endpoint.name] === "function",
    `API client must expose ${endpoint.name}`,
  );

  const args = pathArguments(endpoint.path);
  if (endpoint.method === "POST") {
    args.push({});
  }
  await client[endpoint.name](...args);
}

for (const [index, endpoint] of schema.endpoints.entries()) {
  const call = calls[index];
  assert(call, `API client did not call ${endpoint.name}`);
  assert(!call.path.includes("{"), `${endpoint.name} must replace path parameters`);
  assert(call.options.headers.Authorization === "Bearer typecheck-token", "client must attach bearer token");
  if (endpoint.method === "POST") {
    assert(call.options.method === "POST", `${endpoint.name} must use POST`);
    assert(typeof call.options.body === "string", `${endpoint.name} must JSON encode request bodies`);
  } else {
    assert(!call.options.method, `${endpoint.name} must use default GET`);
  }
}

console.log("web-admin API client type surface check passed");
