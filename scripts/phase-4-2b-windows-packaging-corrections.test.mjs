import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const repoRoot = new URL("../", import.meta.url);

async function readJson(relativePath) {
  return JSON.parse(await readFile(new URL(relativePath, repoRoot), "utf8"));
}

function mergeConfig(base, override) {
  if (
    base &&
    override &&
    typeof base === "object" &&
    typeof override === "object" &&
    !Array.isArray(base) &&
    !Array.isArray(override)
  ) {
    const merged = { ...base };
    for (const [key, value] of Object.entries(override)) {
      merged[key] = key in base ? mergeConfig(base[key], value) : value;
    }
    return merged;
  }
  return override;
}

test("release-like Windows config embeds the offline WebView2 installer", async () => {
  const base = await readJson("src-tauri/tauri.conf.json");
  const windows = await readJson("src-tauri/tauri.windows.conf.json");
  const effective = mergeConfig(base, windows);

  assert.deepEqual(effective.bundle.windows.webviewInstallMode, {
    type: "offlineInstaller",
    silent: true,
  });
  assert.equal(effective.bundle.createUpdaterArtifacts, false);
  assert.equal(effective.plugins?.updater, undefined);

  const templatePath = effective.bundle.windows.nsis?.template;
  assert.equal(
    templatePath,
    "./windows/nsis/installer.nsi",
    "the canonical NSIS build must pin the audited offline-only template",
  );
  const template = await readFile(
    new URL(`src-tauri/${templatePath.replace(/^\.\//, "")}`, repoRoot),
    "utf8",
  );
  assert.doesNotMatch(template, /downloadBootstrapper|NSISdl::download|LinkId=2124703/);
  assert.match(template, /offlineInstaller/);
  assert.match(template, /WEBVIEW2INSTALLERPATH/);
});

test("P4.2b contract freezes NSIS as canonical without deleting MSI", async () => {
  const contract = await readJson(
    "docs/fixtures/phase-4-2b-windows-packaging-corrections/contract.json",
  );

  assert.equal(contract.installer.canonical, "nsis");
  assert.equal(contract.installer.scope, "per_user");
  assert.equal(contract.installer.adminRequiredForNormalInstall, false);
  assert.equal(contract.installer.nonCanonicalArtifacts.includes("msi"), true);
  assert.deepEqual(contract.shortcuts, {
    startMenu: { default: true },
    desktop: { defaultChecked: true, userCanOptOut: true },
  });
  assert.equal(contract.webView2.mode, "offlineInstaller");
  assert.equal(contract.daemon.bundled, true);
  assert.equal(contract.daemon.lookup, "executable-relative");
  assert.equal(contract.daemonctl.required, false);
  assert.equal(contract.installedAcceptance, "NOT_EXECUTED");
});

test("Windows installer release identity is SemVer while Build stays internal", async () => {
  const version = await readJson("VERSION.json");
  const tauri = await readJson("src-tauri/tauri.conf.json");
  const packageJson = await readJson("package.json");

  assert.equal(tauri.version, version.version);
  assert.equal(packageJson.version, version.version);
  assert.equal(version.build, 9);
  assert.equal(tauri.version.split(".").length, 3);
  assert.equal(tauri.version.includes(String(version.build)), false);
});

test("release safety switches remain disabled", async () => {
  const packageJson = await readJson("package.json");
  const tauri = await readJson("src-tauri/tauri.conf.json");
  const main = await readFile(new URL("src/main.tsx", repoRoot), "utf8");

  assert.equal(packageJson.dependencies?.["@tauri-apps/plugin-updater"], undefined);
  assert.equal(packageJson.dependencies?.["@sentry/react"], undefined);
  assert.equal(tauri.bundle.createUpdaterArtifacts, false);
  assert.equal(tauri.plugins?.updater, undefined);
  assert.doesNotMatch(main, /Sentry\.init|captureEvent|captureException/);
});
