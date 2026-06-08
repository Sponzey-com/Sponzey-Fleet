const fs = require("fs");
const path = require("path");

const packageJson = require("../package.json");
const cargoToml = fs.readFileSync(path.join(__dirname, "..", "..", "..", "Cargo.toml"), "utf8");
const versionMatch = cargoToml.match(/\[workspace\.package\][\s\S]*?\nversion\s*=\s*"([^"]+)"/);

if (!versionMatch) {
  console.error("workspace package version was not found in Cargo.toml");
  process.exit(1);
}

const cargoVersion = versionMatch[1];

if (packageJson.version !== cargoVersion) {
  console.error(
    `npm package version ${packageJson.version} does not match Cargo workspace version ${cargoVersion}`,
  );
  process.exit(1);
}

console.log(`npm package version matches Cargo workspace version: ${cargoVersion}`);
