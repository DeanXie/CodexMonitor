import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

const repoRoot = path.resolve(import.meta.dirname, "..");
const fixturesRoot = path.join(
  repoRoot,
  "docs",
  "fixtures",
  "phase-4-1b-release-version-authority",
);

async function loadAuthority() {
  return import(`./version-authority.mjs?test=${Date.now()}-${Math.random()}`);
}

async function readJson(filePath) {
  return JSON.parse(await readFile(filePath, "utf8"));
}

async function makeFixtureRoot(overrides = {}) {
  const root = await mkdtemp(path.join(tmpdir(), "codex-monitor-version-"));
  await mkdir(path.join(root, "src-tauri", "gen", "apple", "codex-monitor_iOS"), {
    recursive: true,
  });
  const version = overrides.version ?? "1.2.3";
  const packageVersion = overrides.packageVersion ?? version;
  const cargoVersion = overrides.cargoVersion ?? version;
  const tauriVersion = overrides.tauriVersion ?? version;
  const lockVersion = overrides.lockVersion ?? version;
  const plistShortVersion = overrides.plistShortVersion ?? version;
  const plistBuild = String(overrides.plistBuild ?? 9);
  await writeFile(
    path.join(root, "VERSION.json"),
    `${JSON.stringify({ version, build: 9, status: "development" }, null, 2)}\n`,
  );
  await writeFile(
    path.join(root, "package.json"),
    `${JSON.stringify({ name: "fixture", version: packageVersion }, null, 2)}\n`,
  );
  await writeFile(
    path.join(root, "package-lock.json"),
    `${JSON.stringify({ name: "fixture", version: packageVersion, lockfileVersion: 3, packages: { "": { name: "fixture", version: packageVersion } } }, null, 2)}\n`,
  );
  await writeFile(
    path.join(root, "src-tauri", "Cargo.toml"),
    `[package]\nname = "codex-monitor"\nversion = "${cargoVersion}"\n`,
  );
  await writeFile(
    path.join(root, "src-tauri", "Cargo.lock"),
    `version = 4\n\n[[package]]\nname = "codex-monitor"\nversion = "${lockVersion}"\n`,
  );
  await writeFile(
    path.join(root, "src-tauri", "tauri.conf.json"),
    `${JSON.stringify({ productName: "Codex Monitor", version: tauriVersion, identifier: "com.dimillian.codexmonitor", bundle: { createUpdaterArtifacts: false } }, null, 2)}\n`,
  );
  await writeFile(
    path.join(root, "src-tauri", "gen", "apple", "codex-monitor_iOS", "Info.plist"),
    `<?xml version="1.0" encoding="UTF-8"?>\n<plist version="1.0"><dict>\n<key>CFBundleShortVersionString</key><string>${plistShortVersion}</string>\n<key>CFBundleVersion</key><string>${plistBuild}</string>\n</dict></plist>\n`,
  );
  return root;
}

async function withFixture(overrides, callback) {
  const root = await makeFixtureRoot(overrides);
  try {
    await callback(root);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

test("canonical_version_manifest_exists", async () => {
  const authority = await readJson(path.join(repoRoot, "VERSION.json"));
  assert.deepEqual(authority, {
    version: "0.7.68",
    build: 5,
    status: "development",
  });
});

test("package_version_matches_canonical", async () => {
  const { checkVersionProjections } = await loadAuthority();
  await withFixture({}, async (root) => {
    assert.deepEqual(await checkVersionProjections(root), []);
  });
});

test("cargo_version_matches_canonical", async () => {
  const { checkVersionProjections } = await loadAuthority();
  await withFixture({}, async (root) => {
    assert.deepEqual(await checkVersionProjections(root), []);
  });
});

test("tauri_version_matches_canonical", async () => {
  const { checkVersionProjections } = await loadAuthority();
  await withFixture({}, async (root) => {
    assert.deepEqual(await checkVersionProjections(root), []);
  });
});

test("version_drift_fails_check", async (context) => {
  const { checkVersionProjections } = await loadAuthority();
  for (const [fixtureName, overrideName, projection] of [
    ["package-drift.json", "packageVersion", "package.json"],
    ["cargo-drift.json", "cargoVersion", "src-tauri/Cargo.toml"],
    ["tauri-drift.json", "tauriVersion", "src-tauri/tauri.conf.json"],
  ]) {
    await context.test(fixtureName, async () => {
      const fixture = await readJson(path.join(fixturesRoot, fixtureName));
      await withFixture({ [overrideName]: fixture.projectionVersion }, async (root) => {
        const mismatches = await checkVersionProjections(root);
        assert.ok(mismatches.some((entry) => entry.projection === projection));
      });
    });
  }
});

test("version_sync_repairs_fixture_drift", async () => {
  const { checkVersionProjections, syncVersionProjections } = await loadAuthority();
  await withFixture(
    {
      packageVersion: "9.9.9",
      cargoVersion: "9.9.9",
      tauriVersion: "9.9.9",
      lockVersion: "9.9.9",
      plistShortVersion: "9.9.9",
      plistBuild: 99,
    },
    async (root) => {
      await syncVersionProjections(root);
      assert.deepEqual(await checkVersionProjections(root), []);
    },
  );
});

test("build_bump_changes_only_build_authority_before_sync", async () => {
  const { bumpVersionAuthority } = await loadAuthority();
  await withFixture({}, async (root) => {
    const packageBefore = await readFile(path.join(root, "package.json"), "utf8");
    const bumped = await bumpVersionAuthority(root, "build");
    assert.deepEqual(bumped, { version: "1.2.3", build: 10, status: "development" });
    assert.equal(await readFile(path.join(root, "package.json"), "utf8"), packageBefore);
  });
});

test("build_bump_is_monotonic", async () => {
  const { bumpVersionAuthority } = await loadAuthority();
  await withFixture({}, async (root) => {
    assert.equal((await bumpVersionAuthority(root, "build")).build, 10);
    assert.equal((await bumpVersionAuthority(root, "build")).build, 11);
  });
});

test("schema_version_is_independent_from_build", async () => {
  const { bumpVersionAuthority } = await loadAuthority();
  const identityBefore = await readJson(path.join(fixturesRoot, "target-release-identity.json"));
  await withFixture({}, async (root) => {
    await bumpVersionAuthority(root, "build");
  });
  const identityAfter = await readJson(path.join(fixturesRoot, "target-release-identity.json"));
  assert.equal(identityBefore.configSchemaVersion, identityAfter.configSchemaVersion);
});

for (const [kind, expectedVersion] of [
  ["patch", "1.2.4"],
  ["minor", "1.3.0"],
  ["major", "2.0.0"],
]) {
  test(`${kind}_bump_changes_A_B_C_correctly`, async () => {
    const { bumpVersionAuthority } = await loadAuthority();
    await withFixture({}, async (root) => {
      const bumped = await bumpVersionAuthority(root, kind);
      assert.equal(bumped.version, expectedVersion);
      assert.equal(bumped.build, 10);
    });
  });
}

test("release_workflow_uses_canonical_version_authority", async () => {
  const workflow = await readFile(path.join(repoRoot, ".github", "workflows", "release.yml"), "utf8");
  assert.match(workflow, /npm run version:check/);
  assert.match(workflow, /VERSION\.json/);
  assert.doesNotMatch(workflow, /Bump version and open PR/);
  assert.doesNotMatch(workflow, /npm version/);
});

test("production_build_has_version_drift_gate", async () => {
  const packageJson = await readJson(path.join(repoRoot, "package.json"));
  const ci = await readFile(path.join(repoRoot, ".github", "workflows", "ci.yml"), "utf8");
  assert.match(packageJson.scripts.prebuild, /version:check/);
  assert.match(packageJson.scripts["pretauri:build"], /version:check/);
  assert.match(packageJson.scripts["pretauri:build:win"], /version:check/);
  assert.match(ci, /npm run version:check/);
});

test("ios_release_build_number_uses_canonical_build", async () => {
  const script = await readFile(path.join(repoRoot, "scripts", "release_testflight_ios.sh"), "utf8");
  assert.match(script, /VERSION\.json/);
  assert.match(script, /v\.build/);
  assert.doesNotMatch(script, /BUILD_NUMBER="\$\(date \+%s\)"/);
});

test("target_release_identity_is_deanx", async () => {
  const identity = await readJson(path.join(repoRoot, "release-identity.json"));
  assert.equal(identity.target.productName, "CodexMonitor DeanX");
  assert.equal(identity.target.desktopIdentifier, "io.github.deanxie.codexmonitor");
  assert.equal(identity.target.iosIdentifier, "io.github.deanxie.codexmonitor.ios");
  assert.equal(identity.publisher.githubOwner, "DeanXie");
  assert.equal(identity.publisher.repository, "CodexMonitor");
  assert.equal(identity.migrationRequired, true);
});

test("target_identifier_is_distinct_from_legacy_identifier", async () => {
  const identity = await readJson(path.join(repoRoot, "release-identity.json"));
  assert.notEqual(identity.target.desktopIdentifier, identity.legacy.desktopIdentifier);
  assert.notEqual(identity.target.iosIdentifier, identity.legacy.iosIdentifier);
});

test("target_desktop_identity_is_activated_while_ios_migration_remains_pending", async () => {
  const identity = await readJson(path.join(repoRoot, "release-identity.json"));
  const desktop = await readJson(path.join(repoRoot, "src-tauri", "tauri.conf.json"));
  const ios = await readJson(path.join(repoRoot, "src-tauri", "tauri.ios.conf.json"));
  const windows = await readJson(path.join(repoRoot, "src-tauri", "tauri.windows.conf.json"));
  assert.equal(desktop.productName, identity.target.productName);
  assert.equal(desktop.identifier, identity.target.desktopIdentifier);
  assert.equal(desktop.app.windows[0].title, identity.target.productName);
  assert.equal(windows.app.windows[0].title, identity.target.productName);
  assert.equal(ios.identifier, identity.legacy.iosIdentifier);
});

test("legacy_runtime_identifier_is_no_longer_the_desktop_authority", async () => {
  const fixture = await readJson(path.join(fixturesRoot, "legacy-runtime-identity.json"));
  const desktop = await readJson(path.join(repoRoot, "src-tauri", "tauri.conf.json"));
  assert.notEqual(desktop.identifier, fixture.desktopIdentifier);
  assert.notEqual(desktop.productName, fixture.productName);
});

test("installer_target_identity_is_stable", async () => {
  const identity = await readJson(path.join(repoRoot, "release-identity.json"));
  const fixture = await readJson(path.join(fixturesRoot, "installer-identity-stability.json"));
  assert.deepEqual(identity.windowsInstaller, fixture.windowsInstaller);
  assert.equal(identity.windowsInstaller.activated, true);
});

test("build_number_does_not_define_semver_precedence", async () => {
  const { compareSoftwareVersions } = await loadAuthority();
  assert.equal(compareSoftwareVersions({ version: "1.2.3", build: 1 }, { version: "1.2.3", build: 999 }), 0);
  assert.equal(compareSoftwareVersions({ version: "1.2.4", build: 1 }, { version: "1.2.3", build: 999 }), 1);
});

test("updater_remains_disabled", async () => {
  const desktop = await readJson(path.join(repoRoot, "src-tauri", "tauri.conf.json"));
  const windows = await readJson(path.join(repoRoot, "src-tauri", "tauri.windows.conf.json"));
  assert.equal(desktop.bundle.createUpdaterArtifacts, false);
  assert.equal(windows.bundle.createUpdaterArtifacts, false);
  assert.equal(desktop.plugins?.updater, undefined);
});

test("sentry_remains_disabled", async () => {
  const packageJson = await readJson(path.join(repoRoot, "package.json"));
  const main = await readFile(path.join(repoRoot, "src", "main.tsx"), "utf8");
  assert.equal(packageJson.dependencies?.["@sentry/react"], undefined);
  assert.doesNotMatch(main, /Sentry\.init|VITE_SENTRY_DSN/);
});

test("desktop_identity_is_active_but_real_data_migration_is_not_executed", async () => {
  const identity = await readJson(path.join(repoRoot, "release-identity.json"));
  assert.equal(identity.migrationRequired, true);
  assert.equal(identity.activation.runtimeIdentity, true);
  assert.equal(identity.activation.dataMigration, false);
});

test("no_real_user_data_is_read", async () => {
  const fixtureFiles = await Promise.all(
    [
      "canonical-version.json",
      "package-drift.json",
      "cargo-drift.json",
      "tauri-drift.json",
      "version-bump.json",
      "target-release-identity.json",
      "legacy-runtime-identity.json",
      "installer-identity-stability.json",
    ].map((name) => readFile(path.join(fixturesRoot, name), "utf8")),
  );
  const inspected = fixtureFiles.join("\n");
  assert.doesNotMatch(inspected, /auth\.json|token|RemoteHostIdentity|C:\\Users\\/i);
});

test("build_trace_is_derived_without_writing_commit_to_VERSION", async () => {
  const { createBuildTrace } = await loadAuthority();
  await withFixture({}, async (root) => {
    const trace = await createBuildTrace(root, "0123456789abcdef");
    assert.deepEqual(trace, {
      version: "1.2.3",
      build: 9,
      status: "development",
      gitCommit: "0123456789abcdef",
    });
    assert.equal((await readJson(path.join(root, "VERSION.json"))).gitCommit, undefined);
  });
});
