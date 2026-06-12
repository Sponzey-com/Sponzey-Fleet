const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");

const bin = path.join(__dirname, "..", "bin", "sponzey");
const postinstall = path.join(__dirname, "postinstall.js");
const packageJson = require(path.join(__dirname, "..", "package.json"));

if (!fs.existsSync(bin)) {
  console.error("missing bin/sponzey");
  process.exit(1);
}

if (packageJson.scripts?.postinstall !== "node ./scripts/postinstall.js") {
  console.error("package.json must run scripts/postinstall.js after npm install");
  process.exit(1);
}

if (!fs.existsSync(postinstall)) {
  console.error("missing scripts/postinstall.js");
  process.exit(1);
}

const body = fs.readFileSync(bin, "utf8");

if (!body.startsWith("#!/usr/bin/env sh")) {
  console.error("bin/sponzey must be a portable shell shim");
  process.exit(1);
}

if (!body.includes("target/debug/sponzey")) {
  console.error("bin/sponzey must point to the Rust development binary");
  process.exit(1);
}

if (!body.includes("SPONZEY_FLEET_BIN")) {
  console.error("bin/sponzey must support explicit binary override for local pack smoke");
  process.exit(1);
}

if (!body.includes("fleet-$PLATFORM_OS-$PLATFORM_ARCH")) {
  console.error("bin/sponzey must support platform binary package lookup");
  process.exit(1);
}

if (!body.includes("node_modules/@sponzey/fleet-$PLATFORM_OS-$PLATFORM_ARCH")) {
  console.error("bin/sponzey must support npm nested optional dependency lookup");
  process.exit(1);
}

const unsupported = spawnSync(bin, ["--help"], {
  env: {
    ...process.env,
    SPONZEY_FLEET_NPM_OS: "plan9",
    SPONZEY_FLEET_NPM_ARCH: "mips",
  },
  encoding: "utf8",
});

if (unsupported.status !== 127) {
  console.error(`unsupported platform should exit 127, got ${unsupported.status}`);
  process.exit(1);
}

if (!unsupported.stderr.includes("unsupported platform for @sponzey/fleet")) {
  console.error("unsupported platform error message is missing");
  process.exit(1);
}

const prefix = fs.mkdtempSync(path.join(os.tmpdir(), "sponzey-postinstall-"));
const pathBin = fs.mkdtempSync(path.join(os.tmpdir(), "sponzey-path-bin-"));
const postinstallRun = spawnSync(process.execPath, [postinstall], {
  env: {
    ...process.env,
    npm_config_global: "true",
    npm_config_prefix: prefix,
    PATH: pathBin,
    SPONZEY_FLEET_POSTINSTALL_LINK_DIRS: pathBin,
  },
  encoding: "utf8",
});

if (postinstallRun.status !== 0) {
  console.error(`postinstall should not fail, got ${postinstallRun.status}`);
  process.exit(1);
}

if (!postinstallRun.stderr.includes("npm global bin is not in PATH")) {
  console.error("postinstall must warn when npm global bin is not in PATH");
  process.exit(1);
}

const installedLauncher = path.join(prefix, "bin", "sponzey");
if (!fs.existsSync(installedLauncher)) {
  console.error("postinstall must create a global sponzey launcher when npm did not");
  process.exit(1);
}

if (!postinstallRun.stderr.includes("sponzey launcher installed at")) {
  console.error("postinstall must show the installed launcher path");
  process.exit(1);
}

const pathVisibleLauncher = path.join(pathBin, "sponzey");
if (!fs.existsSync(pathVisibleLauncher)) {
  console.error("postinstall must create a PATH-visible launcher when possible");
  process.exit(1);
}

if (!postinstallRun.stderr.includes("Created PATH-visible sponzey launcher at")) {
  console.error("postinstall must show the PATH-visible launcher path");
  process.exit(1);
}

console.log("bin/sponzey wrapper checks passed");
