const { arch, platform } = require('node:process');
const { join } = require('node:path');

let nativeBinding: any = null;
let loadError: unknown = null;

function load(file: string): any {
  return require(join(__dirname, 'npm', file));
}

try {
  switch (platform) {
    case 'android':
      if (arch === 'arm64') {
        nativeBinding = load('android-arm64/Rustite.android-arm64.node');
      } else if (arch === 'arm') {
        nativeBinding = load('android-arm-eabi/Rustite.android-arm-eabi.node');
      }
      break;
    case 'darwin':
      if (arch === 'arm64') {
        nativeBinding = load('darwin-arm64/Rustite.darwin-arm64.node');
      } else if (arch === 'x64') {
        nativeBinding = load('darwin-x64/Rustite.darwin-x64.node');
      }
      break;
    case 'linux':
      if (arch === 'arm64') {
        try {
          nativeBinding = load('linux-arm64-gnu/Rustite.linux-arm64-gnu.node');
        } catch {
          nativeBinding = load('linux-arm64-musl/Rustite.linux-arm64-musl.node');
        }
      } else if (arch === 'arm') {
        nativeBinding = load('linux-arm-gnueabihf/Rustite.linux-arm-gnueabihf.node');
      } else if (arch === 'x64') {
        try {
          nativeBinding = load('linux-x64-gnu/Rustite.linux-x64-gnu.node');
        } catch {
          nativeBinding = load('linux-x64-musl/Rustite.linux-x64-musl.node');
        }
      }
      break;
    case 'win32':
      if (arch === 'x64') {
        nativeBinding = load('win32-x64-msvc/Rustite.win32-x64-msvc.node');
      } else if (arch === 'arm64') {
        nativeBinding = load('win32-arm64-msvc/Rustite.win32-arm64-msvc.node');
      }
      break;
  }
} catch (e) {
  loadError = e;
}

if (!nativeBinding) {
  if (loadError) throw loadError;
  throw new Error(`Unsupported platform/architecture: ${platform} ${arch}`);
}

export = nativeBinding;
