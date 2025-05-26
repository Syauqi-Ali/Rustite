const { existsSync } = require('fs');
const { join } = require('path');
const { platform, arch } = require('os');

const filename = `rustite.${platform()}-${arch()}.node`;
const binaryPath = join(__dirname, 'artifacts', filename);

if (!existsSync(binaryPath)) {
  console.error(`Binary for your platform not found: ${binaryPath}`);
  process.exit(1);
}

require('fs').copyFileSync(binaryPath, join(__dirname, 'index.node'));

module.exports = require('./index.node');
