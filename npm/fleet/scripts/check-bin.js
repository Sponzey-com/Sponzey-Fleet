const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");

const bin = path.join(__dirname, "..", "bin", "sponzey");

if (!fs.existsSync(bin)) {
  console.error("missing bin/sponzey");
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

console.log("bin/sponzey wrapper checks passed");
