#!/usr/bin/env node
const fs = require('fs');
const path = require('path');
const os = require('os');

function log(...args) {
  const silent = process.env.npm_config_loglevel === 'silent';
  if (!silent) console.log(...args);
}

function updateSettings() {
  const platform = process.platform;
  const homeDir = os.homedir();

  // Resolve settings.json path
  const settingsPath = platform === 'win32'
    ? path.join(process.env.USERPROFILE || homeDir, '.claude', 'settings.json')
    : path.join(homeDir, '.claude', 'settings.json');

  // Desired command path (match where postinstall puts the binary)
  const desiredCommand = platform === 'win32'
    ? '%USERPROFILE%\\.claude\\ccstatus\\ccstatus.exe'
    : '~/.claude/ccstatus/ccstatus';

  // Desired statusLine block
  const desiredStatusLine = {
    type: 'command',
    command: desiredCommand,
    padding: 0,
  };

  try {
    // Ensure parent directory exists
    fs.mkdirSync(path.dirname(settingsPath), { recursive: true });

    let current = {};
    if (fs.existsSync(settingsPath)) {
      try {
        const raw = fs.readFileSync(settingsPath, 'utf8');
        current = JSON.parse(raw);
      } catch (e) {
        const backup = settingsPath + '.bak-' + new Date().toISOString().replace(/[:]/g, '-');
        fs.copyFileSync(settingsPath, backup);
        log(`Existing settings.json is not valid JSON. Backed up to: ${backup}`);
        // Start fresh rather than writing over an unparsable file
        current = {};
      }
    }

    const before = JSON.stringify(current.statusLine);

    // Set/overwrite statusLine
    current.statusLine = desiredStatusLine;

    const after = JSON.stringify(current.statusLine);
    if (before === after) {
      log('Claude settings.json already up-to-date for CCstatus.');
      return { updated: false, settingsPath };
    }

    fs.writeFileSync(settingsPath, JSON.stringify(current, null, 2) + '\n');
    log(`Updated Claude settings at: ${settingsPath}`);
    return { updated: true, settingsPath };
  } catch (err) {
    log('Warning: Failed to update Claude settings.json automatically:', err.message);
    return { updated: false, error: err };
  }
}

function main() {
  updateSettings();
}

module.exports = { run: updateSettings };

if (require.main === module) {
  main();
}
