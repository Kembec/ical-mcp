#!/usr/bin/env node

/**
 * Postinstall script - downloads the JAR from GitHub Releases
 *
 * Security note: The downloaded JAR is not verified against a pinned checksum
 * because this fork may be installed directly from source. Instead, the SHA-256
 * of every download is recorded to a sidecar file (<JAR_PATH>.sha256) for
 * manual audit. Before running in production, verify this hash against the
 * upstream GitHub Release or build the JAR locally with `./gradlew fatJar`.
 */

const https = require('https');
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const pkg = require('../package.json');
const VERSION = pkg.version;
const JAR_NAME = `icloud-calendar-mcp-${VERSION}-all.jar`;
const DOWNLOAD_URL = `https://github.com/icloud-calendar-mcp/icloud-calendar-mcp/releases/download/v${VERSION}/${JAR_NAME}`;
const LIB_DIR = path.join(__dirname);
const JAR_PATH = path.join(LIB_DIR, JAR_NAME);

// ANSI colors
const GREEN = '\x1b[32m';
const YELLOW = '\x1b[33m';
const RED = '\x1b[31m';
const RESET = '\x1b[0m';

function log(msg) {
  console.log(msg);
}

function downloadFile(url, dest) {
  return new Promise((resolve, reject) => {
    log(`${YELLOW}Downloading iCloud Calendar MCP Server...${RESET}`);
    log(`URL: ${url}`);

    const file = fs.createWriteStream(dest);
    let downloadedBytes = 0;

    const request = https.get(url, (response) => {
      // Handle redirects (GitHub releases redirect to S3)
      if (response.statusCode === 302 || response.statusCode === 301) {
        file.close();
        fs.unlinkSync(dest);
        return downloadFile(response.headers.location, dest).then(resolve).catch(reject);
      }

      if (response.statusCode !== 200) {
        file.close();
        fs.unlinkSync(dest);
        reject(new Error(`Download failed: HTTP ${response.statusCode}`));
        return;
      }

      const totalBytes = parseInt(response.headers['content-length'], 10);

      response.on('data', (chunk) => {
        downloadedBytes += chunk.length;
        if (totalBytes) {
          const percent = Math.round((downloadedBytes / totalBytes) * 100);
          process.stdout.write(`\rDownloading: ${percent}% (${(downloadedBytes / 1024 / 1024).toFixed(1)} MB)`);
        }
      });

      response.pipe(file);

      file.on('finish', () => {
        file.close();
        console.log(''); // newline after progress
        log(`${GREEN}Download complete!${RESET}`);
        resolve();
      });
    });

    request.on('error', (err) => {
      file.close();
      fs.unlinkSync(dest);
      reject(err);
    });

    file.on('error', (err) => {
      file.close();
      fs.unlinkSync(dest);
      reject(err);
    });
  });
}

async function main() {
  // Skip if JAR already exists (e.g., from cache or bundled)
  if (fs.existsSync(JAR_PATH)) {
    const stats = fs.statSync(JAR_PATH);
    if (stats.size > 1000000) { // > 1MB means it's a real JAR
      log(`${GREEN}JAR already present, skipping download.${RESET}`);
      return;
    }
  }

  // Ensure lib directory exists
  if (!fs.existsSync(LIB_DIR)) {
    fs.mkdirSync(LIB_DIR, { recursive: true });
  }

  try {
    await downloadFile(DOWNLOAD_URL, JAR_PATH);

    // Verify the download
    const stats = fs.statSync(JAR_PATH);
    if (stats.size < 1000000) {
      throw new Error('Downloaded file is too small, may be corrupted');
    }

    // Warn if the JAR is unexpectedly small for a fat JAR (typically 15–40 MB).
    // A file under 5 MB likely indicates a partial download or a stub artifact.
    const MIN_FAT_JAR_BYTES = 5 * 1024 * 1024; // 5 MB
    if (stats.size < MIN_FAT_JAR_BYTES) {
      log(`${YELLOW}WARNING: Downloaded JAR is ${(stats.size / 1024 / 1024).toFixed(1)} MB, which is smaller than expected for a fat JAR (>5 MB). Verify the file before use.${RESET}`);
    }

    // Record SHA-256 of the downloaded JAR to a sidecar file for manual audit.
    // This does NOT verify against a pinned hash — it creates an audit trail so
    // you can confirm the same artifact is used across environments.
    // To verify: compare <JAR>.sha256 against the upstream GitHub Release or
    // against a locally built JAR (`./gradlew fatJar`).
    const jarBuffer = fs.readFileSync(JAR_PATH);
    const sha256 = crypto.createHash('sha256').update(jarBuffer).digest('hex');
    const sidecarPath = `${JAR_PATH}.sha256`;
    fs.writeFileSync(sidecarPath, `${sha256}  ${path.basename(JAR_PATH)}\n`);
    log(`${GREEN}SHA-256 recorded: ${sha256}${RESET}`);
    log(`Sidecar written to: ${sidecarPath}`);

    log(`${GREEN}iCloud Calendar MCP Server installed successfully!${RESET}`);
    log(`\nUsage: npx @icloud-calendar-mcp/server --help`);
  } catch (err) {
    log(`${RED}Error downloading JAR:${RESET} ${err.message}`);
    log(`\nYou can manually download from:`);
    log(`  ${DOWNLOAD_URL}`);
    log(`And place it at:`);
    log(`  ${JAR_PATH}`);
    process.exit(1);
  }
}

main();
