import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const args = process.argv.slice(2);
if (args.length && (args.length !== 2 || args[0] !== "--notes-file")) {
  throw new Error("Usage: npm run build:release -- [--notes-file <path>]");
}
const config = JSON.parse(readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8"));
const packageJson = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const cargoToml = readFileSync(join(root, "src-tauri/Cargo.toml"), "utf8");
const version = config.version;
if (packageJson.version !== version || !cargoToml.includes(`version = "${version}"`)) {
  throw new Error("Synchronize the versions in package.json, Cargo.toml and tauri.conf.json before building.");
}
if (!config.bundle.createUpdaterArtifacts || !config.plugins?.updater?.pubkey) {
  throw new Error("Updater artifact generation and a public signing key must be configured.");
}
const notes = args.length
  ? readFileSync(resolve(root, args[1]), "utf8")
  : `Backup Sync ${version}. See the GitHub release notes for details.`;

const env = { ...process.env };
if (!env.TAURI_SIGNING_PRIVATE_KEY) {
  if (!env.LOCALAPPDATA) throw new Error("Set TAURI_SIGNING_PRIVATE_KEY to the updater signing key.");
  const keyPath = join(env.LOCALAPPDATA, "BackupSync/signing/updater.key");
  if (!existsSync(keyPath)) throw new Error("Updater signing key not found. See docs/app-updates.md.");
  env.TAURI_SIGNING_PRIVATE_KEY = keyPath;
}
env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ??= "";

const cli = join(root, "node_modules/@tauri-apps/cli/tauri.js");
const build = spawnSync(process.execPath, [cli, "build", "--bundles", "nsis"], {
  cwd: root,
  env,
  stdio: "inherit",
});
if (build.error) throw build.error;
if (build.status !== 0) process.exit(build.status ?? 1);

const bundle = join(root, "src-tauri/target/release/bundle/nsis");
const builtName = `Backup Sync_${version}_x64-setup.exe`;
const installer = readFileSync(join(bundle, builtName));
const signature = readFileSync(join(bundle, `${builtName}.sig`), "utf8").trim();
const trustedComment = Buffer.from(signature, "base64").toString("utf8")
  .split(/\r?\n/).find((line) => line.startsWith("trusted comment:"));
if (!trustedComment?.split("\t").includes(`version:${version}`)) {
  throw new Error("The updater signature must include the release version. Update the Tauri CLI and rebuild.");
}
const destination = join(root, "release-artifacts", `v${version}`);
mkdirSync(destination, { recursive: true });
const assetName = `Backup.Sync_${version}_x64-setup.exe`;
copyFileSync(join(bundle, builtName), join(destination, assetName));
writeFileSync(join(destination, `${assetName}.sig`), `${signature}\n`);
const hash = createHash("sha256").update(installer).digest("hex");
writeFileSync(join(destination, `${assetName}.sha256`), `${hash}  ${assetName}\n`);
const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": {
      signature,
      url: `https://github.com/ValentinSK0/backup-sync/releases/download/v${version}/${assetName}`,
    },
  },
};
writeFileSync(join(destination, "latest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`Release assets ready in ${destination}`);
console.log(`Installer ${installer.length} bytes, SHA-256 ${hash}`);
console.log("Upload the installer, .sig, .sha256 and latest.json to the matching GitHub release.");
