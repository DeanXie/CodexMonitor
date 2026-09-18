import { execFileSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const VERSION_PATTERN = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const VALID_STATUSES = new Set(["development", "released"]);

const PROJECTIONS = [
  "package.json",
  "package-lock.json",
  "src-tauri/Cargo.toml",
  "src-tauri/Cargo.lock",
  "src-tauri/tauri.conf.json",
  "src-tauri/gen/apple/codex-monitor_iOS/Info.plist",
];

async function readJson(filePath) {
  return JSON.parse(await readFile(filePath, "utf8"));
}

async function writeJson(filePath, value) {
  await writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function parseVersion(version) {
  const match = VERSION_PATTERN.exec(version);
  if (!match) {
    throw new Error(`Invalid software version ${JSON.stringify(version)}; expected A.B.C`);
  }
  return match.slice(1).map(Number);
}

function validateAuthority(authority) {
  const keys = Object.keys(authority).sort();
  const expectedKeys = ["build", "status", "version"];
  if (JSON.stringify(keys) !== JSON.stringify(expectedKeys)) {
    throw new Error(`VERSION.json must contain exactly: ${expectedKeys.join(", ")}`);
  }
  parseVersion(authority.version);
  if (!Number.isSafeInteger(authority.build) || authority.build < 1) {
    throw new Error("VERSION.json build must be a positive integer");
  }
  if (!VALID_STATUSES.has(authority.status)) {
    throw new Error('VERSION.json status must be "development" or "released"');
  }
  return authority;
}

export async function readVersionAuthority(root) {
  return validateAuthority(await readJson(path.join(root, "VERSION.json")));
}

function cargoPackageVersion(contents) {
  const match = /^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m.exec(contents);
  if (!match) throw new Error("Unable to locate [package] version in Cargo.toml");
  return match[1];
}

function cargoLockPackageVersion(contents) {
  const match = /\[\[package\]\]\s*\r?\nname\s*=\s*"codex-monitor"\s*\r?\nversion\s*=\s*"([^"]+)"/m.exec(contents);
  if (!match) throw new Error("Unable to locate codex-monitor version in Cargo.lock");
  return match[1];
}

function plistValue(contents, key) {
  const pattern = new RegExp(`<key>${key}</key>\\s*<string>([^<]+)</string>`);
  const match = pattern.exec(contents);
  if (!match) throw new Error(`Unable to locate ${key} in Info.plist`);
  return match[1];
}

function replaceCargoPackageVersion(contents, version) {
  const replaced = contents.replace(
    /(^\[package\][\s\S]*?^version\s*=\s*")[^"]+(".*$)/m,
    `$1${version}$2`,
  );
  if (replaced === contents && cargoPackageVersion(contents) !== version) {
    throw new Error("Failed to update Cargo.toml package version");
  }
  return replaced;
}

function replaceCargoLockPackageVersion(contents, version) {
  const replaced = contents.replace(
    /(\[\[package\]\]\s*\r?\nname\s*=\s*"codex-monitor"\s*\r?\nversion\s*=\s*")[^"]+("\s*)/m,
    `$1${version}$2`,
  );
  if (replaced === contents && cargoLockPackageVersion(contents) !== version) {
    throw new Error("Failed to update Cargo.lock codex-monitor version");
  }
  return replaced;
}

function replacePlistValue(contents, key, value) {
  const pattern = new RegExp(`(<key>${key}</key>\\s*<string>)[^<]+(</string>)`);
  if (!pattern.test(contents)) throw new Error(`Unable to locate ${key} in Info.plist`);
  return contents.replace(pattern, `$1${value}$2`);
}

async function projectionValues(root) {
  const packageJson = await readJson(path.join(root, "package.json"));
  const packageLock = await readJson(path.join(root, "package-lock.json"));
  const cargoToml = await readFile(path.join(root, "src-tauri", "Cargo.toml"), "utf8");
  const cargoLock = await readFile(path.join(root, "src-tauri", "Cargo.lock"), "utf8");
  const tauri = await readJson(path.join(root, "src-tauri", "tauri.conf.json"));
  const infoPlist = await readFile(
    path.join(root, "src-tauri", "gen", "apple", "codex-monitor_iOS", "Info.plist"),
    "utf8",
  );
  return [
    ["package.json", packageJson.version, "version"],
    ["package-lock.json", packageLock.version, "version"],
    ["package-lock.json#packages-root", packageLock.packages?.[""]?.version, "version"],
    ["src-tauri/Cargo.toml", cargoPackageVersion(cargoToml), "version"],
    ["src-tauri/Cargo.lock", cargoLockPackageVersion(cargoLock), "version"],
    ["src-tauri/tauri.conf.json", tauri.version, "version"],
    ["src-tauri/gen/apple/codex-monitor_iOS/Info.plist#CFBundleShortVersionString", plistValue(infoPlist, "CFBundleShortVersionString"), "version"],
    ["src-tauri/gen/apple/codex-monitor_iOS/Info.plist#CFBundleVersion", plistValue(infoPlist, "CFBundleVersion"), "build"],
  ];
}

export async function checkVersionProjections(root) {
  const authority = await readVersionAuthority(root);
  const expectedByKind = { version: authority.version, build: String(authority.build) };
  const values = await projectionValues(root);
  return values
    .filter(([, actual, kind]) => actual !== expectedByKind[kind])
    .map(([projection, actual, kind]) => ({
      projection,
      expected: expectedByKind[kind],
      actual: actual ?? null,
    }));
}

export async function syncVersionProjections(root) {
  const authority = await readVersionAuthority(root);
  const packagePath = path.join(root, "package.json");
  const packageJson = await readJson(packagePath);
  packageJson.version = authority.version;
  await writeJson(packagePath, packageJson);

  const lockPath = path.join(root, "package-lock.json");
  const packageLock = await readJson(lockPath);
  packageLock.version = authority.version;
  if (!packageLock.packages?.[""]) throw new Error("package-lock.json has no root package");
  packageLock.packages[""].version = authority.version;
  await writeJson(lockPath, packageLock);

  const cargoPath = path.join(root, "src-tauri", "Cargo.toml");
  await writeFile(
    cargoPath,
    replaceCargoPackageVersion(await readFile(cargoPath, "utf8"), authority.version),
    "utf8",
  );

  const cargoLockPath = path.join(root, "src-tauri", "Cargo.lock");
  await writeFile(
    cargoLockPath,
    replaceCargoLockPackageVersion(await readFile(cargoLockPath, "utf8"), authority.version),
    "utf8",
  );

  const tauriPath = path.join(root, "src-tauri", "tauri.conf.json");
  const tauri = await readJson(tauriPath);
  tauri.version = authority.version;
  await writeJson(tauriPath, tauri);

  const plistPath = path.join(root, "src-tauri", "gen", "apple", "codex-monitor_iOS", "Info.plist");
  let plist = await readFile(plistPath, "utf8");
  plist = replacePlistValue(plist, "CFBundleShortVersionString", authority.version);
  plist = replacePlistValue(plist, "CFBundleVersion", String(authority.build));
  await writeFile(plistPath, plist, "utf8");

  return PROJECTIONS;
}

export async function bumpVersionAuthority(root, kind) {
  if (!["build", "patch", "minor", "major"].includes(kind)) {
    throw new Error("Bump kind must be one of: build, patch, minor, major");
  }
  const authority = await readVersionAuthority(root);
  let [major, minor, patch] = parseVersion(authority.version);
  if (kind === "patch") patch += 1;
  if (kind === "minor") {
    minor += 1;
    patch = 0;
  }
  if (kind === "major") {
    major += 1;
    minor = 0;
    patch = 0;
  }
  const next = {
    version: `${major}.${minor}.${patch}`,
    build: authority.build + 1,
    status: authority.status,
  };
  await writeJson(path.join(root, "VERSION.json"), next);
  return next;
}

export function compareSoftwareVersions(left, right) {
  const leftParts = parseVersion(left.version);
  const rightParts = parseVersion(right.version);
  for (let index = 0; index < 3; index += 1) {
    if (leftParts[index] > rightParts[index]) return 1;
    if (leftParts[index] < rightParts[index]) return -1;
  }
  return 0;
}

export async function createBuildTrace(root, gitCommit) {
  const authority = await readVersionAuthority(root);
  return { ...authority, gitCommit };
}

function resolveGitCommit(root) {
  if (process.env.GITHUB_SHA) return process.env.GITHUB_SHA;
  return execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim();
}

async function main() {
  const root = path.resolve(import.meta.dirname, "..");
  const [command, argument] = process.argv.slice(2);
  if (command === "check") {
    const mismatches = await checkVersionProjections(root);
    if (mismatches.length) {
      console.error(JSON.stringify({ status: "VERSION_DRIFT", mismatches }, null, 2));
      process.exitCode = 1;
      return;
    }
    const authority = await readVersionAuthority(root);
    console.log(`Version projections match v${authority.version} · Build ${authority.build}.`);
    return;
  }
  if (command === "sync") {
    await syncVersionProjections(root);
    const authority = await readVersionAuthority(root);
    console.log(`Synchronized projections to v${authority.version} · Build ${authority.build}.`);
    return;
  }
  if (command === "bump") {
    const authority = await bumpVersionAuthority(root, argument);
    console.log(JSON.stringify(authority, null, 2));
    console.log("Run npm run version:sync explicitly before building.");
    return;
  }
  if (command === "trace") {
    console.log(JSON.stringify(await createBuildTrace(root, resolveGitCommit(root)), null, 2));
    return;
  }
  throw new Error("Usage: version-authority.mjs <check|sync|bump KIND|trace>");
}

const isCli = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isCli) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
