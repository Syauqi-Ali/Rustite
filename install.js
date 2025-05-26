const { existsSync, copyFileSync } = require("fs");
const { join } = require("path");
const os = require("os");

if (process.env.CI === "true") {
  console.log("Skipping install script in CI environment.");
  process.exit(0);
}

const platform = os.platform();
const arch = os.arch();

const binaryName = `rustite.${platform}-${arch}.node`;
const sourcePath = join(__dirname, "artifacts", binaryName);
const targetPath = join(__dirname, "index.node");

if (!existsSync(sourcePath)) {
  console.error(`Prebuilt binary not found: ${binaryName}`);
  process.exit(1);
}

copyFileSync(sourcePath, targetPath);
console.log(`Copied prebuilt binary: ${binaryName}`);

module.exports = require('./index.node');