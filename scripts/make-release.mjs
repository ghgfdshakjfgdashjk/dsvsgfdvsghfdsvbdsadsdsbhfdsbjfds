import { readFileSync, writeFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const REPO = "Boots3453/BootsAutoClicker";

const config = JSON.parse(readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8"));
const { version } = config;

const bundleDir = join(root, "src-tauri/target/release/bundle/nsis");

let files;
try {
  files = readdirSync(bundleDir);
} catch {
  console.error(`No bundle found at ${bundleDir}\nRun "npm run tauri build" first.`);
  process.exit(1);
}

// Old builds stay in this folder, so match on the version being released --
// picking merely "an installer" can pick up a previous one and point the
// updater at a file that is not on the release.
const installer = files.find((f) => f.includes(version) && f.endsWith("-setup.exe"));
const signatureFile = files.find((f) => f.includes(version) && f.endsWith("-setup.exe.sig"));

if (!installer) {
  console.error(
    `No installer for ${version} in the bundle folder — the build didn't finish.\n` +
      `Found: ${files.filter((f) => f.endsWith("-setup.exe")).join(", ") || "nothing"}`,
  );
  process.exit(1);
}

if (!signatureFile) {
  console.error(
    "No .sig file. The build ran without a signing key, so this release " +
      "can't be used as an update.\n" +
      "Set TAURI_SIGNING_PRIVATE_KEY (a path to the key is fine) and\n" +
      "TAURI_SIGNING_PRIVATE_KEY_PASSWORD, then build again.",
  );
  process.exit(1);
}

const manifest = {
  version,
  notes: process.argv[2] ?? `Syntax ${version}`,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": {
      signature: readFileSync(join(bundleDir, signatureFile), "utf8").trim(),
      url: `https://github.com/${REPO}/releases/download/v${version}/${encodeURIComponent(installer)}`,
    },
  },
};

const output = join(bundleDir, "latest.json");
writeFileSync(output, JSON.stringify(manifest, null, 2));

console.log(`\nWrote ${output}\n`);
console.log(`Create a GitHub release tagged  v${version}  and upload:`);
console.log(`  - ${installer}`);
console.log(`  - latest.json`);
console.log(`\nBoth live in: ${bundleDir}\n`);
