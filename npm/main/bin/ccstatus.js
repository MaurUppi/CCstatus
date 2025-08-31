#!/usr/bin/env node
const { spawnSync } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');

// 1. Priority: Use ~/.claude/ccstatus/ccstatus if exists
const claudePath = path.join(
  os.homedir(), 
  '.claude', 
  'ccstatus',
  process.platform === 'win32' ? 'ccstatus.exe' : 'ccstatus'
);

if (fs.existsSync(claudePath)) {
  const result = spawnSync(claudePath, process.argv.slice(2), {
    stdio: 'inherit',
    shell: false
  });
  process.exit(result.status || 0);
}

// 2. Fallback: Use npm package binary
const platform = process.platform;
const arch = process.arch;

// Handle special cases
const platformKey = `${platform}-${arch}`;

const packageMap = {
  'darwin-x64': '@mauruppi/ccstatus-darwin-x64',
  'darwin-arm64': '@mauruppi/ccstatus-darwin-arm64',
  'linux-x64': '@mauruppi/ccstatus-linux-x64',
  'win32-x64': '@mauruppi/ccstatus-win32-x64',
  'win32-ia32': '@mauruppi/ccstatus-win32-x64', // Use 64-bit for 32-bit systems
};

const packageName = packageMap[platformKey];
if (!packageName) {
  console.error(`Error: Unsupported platform ${platformKey}`);
  console.error('Supported platforms: darwin (x64/arm64), linux (x64), win32 (x64)');
  console.error('Please visit https://github.com/MaurUppi/CCstatus for manual installation');
  process.exit(1);
}

const binaryName = platform === 'win32' ? 'ccstatus.exe' : 'ccstatus';
const binaryPath = path.join(__dirname, '..', 'node_modules', packageName, binaryName);

if (!fs.existsSync(binaryPath)) {
  console.error(`Error: Binary not found at ${binaryPath}`);
  console.error('This might indicate a failed installation or unsupported platform.');
  console.error('Please try reinstalling: npm install -g @mauruppi/ccstatus');
  console.error(`Expected package: ${packageName}`);
  process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: 'inherit',
  shell: false
});

process.exit(result.status || 0);