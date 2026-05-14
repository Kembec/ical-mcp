#!/usr/bin/env node
const { execFileSync } = require('child_process');

const PLATFORMS = {
  'darwin-arm64': '@kembec/ical-mcp-darwin-arm64',
  'darwin-x64': '@kembec/ical-mcp-darwin-x64',
  'linux-x64': '@kembec/ical-mcp-linux-x64',
  'win32-x64': '@kembec/ical-mcp-win32-x64',
};

const key = `${process.platform}-${process.arch}`;
const pkg = PLATFORMS[key];
if (!pkg) {
  console.error(`ical-mcp: unsupported platform ${key}`);
  process.exit(1);
}

let binPath;
try {
  const binName = process.platform === 'win32' ? 'ical-mcp.exe' : 'ical-mcp';
  binPath = require.resolve(`${pkg}/bin/${binName}`);
} catch {
  console.error(
    `ical-mcp: platform package ${pkg} is not installed. ` +
    `Re-run \`npm install ${'@kembec/ical-mcp'}\`.`
  );
  process.exit(1);
}

try {
  execFileSync(binPath, process.argv.slice(2), { stdio: 'inherit' });
} catch (e) {
  process.exit(typeof e.status === 'number' ? e.status : 1);
}
