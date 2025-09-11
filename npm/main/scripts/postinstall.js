const fs = require('fs');
const path = require('path');
const os = require('os');

// Silent mode detection
const silent = process.env.npm_config_loglevel === 'silent' || 
               process.env.CCSTATUS_SKIP_POSTINSTALL === '1';

if (!silent) {
  console.log('🚀 Setting up CCstatus for Claude Code...');
}

try {
  const platform = process.platform;
  const arch = process.arch;
  const homeDir = os.homedir();
  const claudeDir = path.join(homeDir, '.claude', 'ccstatus');

  // Create directory
  fs.mkdirSync(claudeDir, { recursive: true });

  // Determine platform key
  const platformKey = `${platform}-${arch}`;

  const packageMap = {
    'darwin-x64': '@mauruppi/ccstatus-darwin-x64',
    'darwin-arm64': '@mauruppi/ccstatus-darwin-arm64',
    'linux-x64': '@mauruppi/ccstatus-linux-x64',
    'win32-x64': '@mauruppi/ccstatus-win32-x64',
    'win32-ia32': '@mauruppi/ccstatus-win32-x64', // Use 64-bit for 32-bit
  };

  const packageName = packageMap[platformKey];
  if (!packageName) {
    if (!silent) {
      console.log(`Platform ${platformKey} not supported for auto-setup`);
    }
    process.exit(0);
  }

  const binaryName = platform === 'win32' ? 'ccstatus.exe' : 'ccstatus';
  const targetPath = path.join(claudeDir, binaryName);

  // Multiple path search strategies for different package managers
  const findBinaryPath = () => {
    const possiblePaths = [
      // npm/yarn: nested in node_modules
      path.join(__dirname, '..', 'node_modules', packageName, binaryName),
      // pnpm: try require.resolve first
      (() => {
        try {
          const packagePath = require.resolve(packageName + '/package.json');
          return path.join(path.dirname(packagePath), binaryName);
        } catch {
          return null;
        }
      })(),
      // pnpm: flat structure fallback with version detection
      (() => {
        const currentPath = __dirname;
        const pnpmMatch = currentPath.match(/(.+\.pnpm)[\\/]([^\\//]+)[\\/]/);
        if (pnpmMatch) {
          const pnpmRoot = pnpmMatch[1];
          const packageNameEncoded = packageName.replace('/', '+');
          
          try {
            // Try to find any version of the package
            const pnpmContents = fs.readdirSync(pnpmRoot);
            const packagePattern = new RegExp(`^${packageNameEncoded.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}@`);
            const matchingPackage = pnpmContents.find(dir => packagePattern.test(dir));
            
            if (matchingPackage) {
              return path.join(pnpmRoot, matchingPackage, 'node_modules', packageName, binaryName);
            }
          } catch {
            // Fallback to current behavior if directory reading fails
          }
        }
        return null;
      })()
    ].filter(p => p !== null);

    for (const testPath of possiblePaths) {
      if (fs.existsSync(testPath)) {
        return testPath;
      }
    }
    return null;
  };

  const sourcePath = findBinaryPath();
  if (!sourcePath) {
    if (!silent) {
      console.log('Binary package not installed, skipping Claude Code setup');
      console.log('The global ccstatus command will still work via npm');
    }
    process.exit(0);
  }

  // Copy or link the binary
  if (platform === 'win32') {
    // Windows: Copy file
    fs.copyFileSync(sourcePath, targetPath);
  } else {
    // Unix: Try hard link first, fallback to copy
    try {
      if (fs.existsSync(targetPath)) {
        fs.unlinkSync(targetPath);
      }
      fs.linkSync(sourcePath, targetPath);
    } catch {
      fs.copyFileSync(sourcePath, targetPath);
    }
    fs.chmodSync(targetPath, '755');
  }

  if (!silent) {
    console.log('✨ CCstatus is ready for Claude Code!');
    console.log(`📍 Location: ${targetPath}`);
    console.log('🎉 You can now use: ccstatus --help');
  }

  // Attempt to update Claude settings.json with statusLine configuration
  try {
    const updater = require('./update-settings');
    const res = updater.run();
    if (!silent && res && res.settingsPath) {
      console.log(`🔧 Claude settings configured at: ${res.settingsPath}`);
    }
  } catch (e) {
    if (!silent) {
      console.log('Note: Failed to update Claude settings.json automatically.');
    }
  }
} catch (error) {
  // Silent failure - don't break installation
  if (!silent) {
    console.log('Note: Could not auto-configure for Claude Code');
    console.log('The global ccstatus command will still work.');
    console.log('You can manually copy ccstatus to ~/.claude/ccstatus/ if needed');
  }
}
