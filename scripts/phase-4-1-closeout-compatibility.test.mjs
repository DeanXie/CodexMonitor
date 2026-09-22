import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const repoRoot = path.resolve(import.meta.dirname, "..");

async function readJson(relativePath) {
  return JSON.parse(await readFile(path.join(repoRoot, relativePath), "utf8"));
}

async function readText(relativePath) {
  return readFile(path.join(repoRoot, relativePath), "utf8");
}

function mergeConfig(base, override) {
  if (Array.isArray(override) || override === null || typeof override !== "object") {
    return override;
  }
  const merged = { ...base };
  for (const [key, value] of Object.entries(override)) {
    const current = merged[key];
    merged[key] =
      value !== null &&
      typeof value === "object" &&
      !Array.isArray(value) &&
      current !== null &&
      typeof current === "object" &&
      !Array.isArray(current)
        ? mergeConfig(current, value)
        : value;
  }
  return merged;
}

async function loadCloseoutContract() {
  return readJson("docs/fixtures/phase-4-1-closeout/contract.json");
}

test("p4_1_final_status_and_compliance_inventory_are_frozen", async () => {
  const contract = await loadCloseoutContract();

  assert.equal(contract.closeoutStatus, "PASS_COMPLETE_FROZEN");
  assert.deepEqual(contract.phaseStatus, {
    p4_1d: "PASS_COMPLETE_FROZEN",
    p4_1e: "PASS_COMPLETE_FROZEN",
    p4_1: "PASS_COMPLETE_FROZEN",
    p4_2: "NOT_STARTED",
  });
  assert.deepEqual(contract.resolvedCompliance, [
    "R01", "R02", "R03", "R04", "R05", "R06",
    "R07", "R08", "R09", "R10", "R11", "R12",
  ]);
  assert.deepEqual(contract.remainingCompliance, []);
});

test("d4c_freezes_native_gate_restart_and_process_isolation", async () => {
  const contract = await readJson(
    "docs/fixtures/phase-4-1d-4c-entry-isolation-closeout/contract.json",
  );
  assert.equal(contract.status, "pass_complete_frozen");
  assert.equal(contract.nativeBusinessGate.menuBusinessEmissionBeforeReady, 0);
  assert.equal(contract.nativeBusinessGate.trayBusinessDispatchBeforeReady, 0);
  assert.equal(contract.nativeBusinessGate.globalSourceStartBeforeReady, 0);
  assert.equal(contract.nativeBusinessGate.daemonManagementStartBeforeReady, 0);
  assert.equal(contract.nativeBusinessGate.windowsTrayAdded, false);
  assert.equal(contract.activationUx.decision, "restart_required");
  assert.equal(contract.activationUx.hotSwitch, false);
  assert.equal(contract.activationUx.nextProcessMustValidate, true);
  assert.equal(contract.executedTestIsolation.daemonTokenRemovedByDefault, true);
  assert.equal(contract.executedTestIsolation.realUserDataReads, 0);
  assert.equal(contract.executedTestIsolation.realUserServiceOperations, 0);
});

test("updater_sentry_remain_disabled", async () => {
  const packageJson = await readJson("package.json");
  const base = await readJson("src-tauri/tauri.conf.json");
  const windows = await readJson("src-tauri/tauri.windows.conf.json");
  const mergedWindows = mergeConfig(base, windows);
  const cargo = await readText("src-tauri/Cargo.toml");
  const main = await readText("src/main.tsx");
  const workflow = await readText(".github/workflows/release.yml");

  assert.equal(base.bundle.createUpdaterArtifacts, false);
  assert.equal(mergedWindows.bundle.createUpdaterArtifacts, false);
  assert.equal(base.plugins?.updater, undefined);
  assert.equal(mergedWindows.plugins?.updater, undefined);
  assert.equal(packageJson.dependencies?.["@tauri-apps/plugin-updater"], undefined);
  assert.equal(packageJson.dependencies?.["@sentry/react"], undefined);
  assert.doesNotMatch(cargo, /tauri-plugin-updater|tauri-plugin-process/);
  assert.doesNotMatch(main, /Sentry\.init|captureException|captureMessage/);
  assert.doesNotMatch(workflow, /latest\.json|TAURI_SIGNING_PRIVATE_KEY|tauri signer sign/);
  assert.match(workflow, /windows-artifacts/);
  assert.match(workflow, /release-artifacts\/release-notes\.md/);
});

test("version_authority_stable_and_schema_independent", async () => {
  const contract = await loadCloseoutContract();
  const version = await readJson("VERSION.json");
  const releaseIdentity = await readJson("release-identity.json");
  assert.equal(version.version, contract.versionAuthority.version);
  assert.equal(version.status, contract.versionAuthority.status);
  assert.ok(
    version.build >= contract.versionAuthority.build,
    "current monotonic Build must not regress below the frozen P4.1 baseline",
  );
  assert.equal(releaseIdentity.configSchemaVersion, contract.configSchemaVersion);
  assert.notEqual(version.build, releaseIdentity.configSchemaVersion);
});

test("release_identity_consistent_across_effective_platform_configs", async () => {
  const identity = await readJson("release-identity.json");
  const base = await readJson("src-tauri/tauri.conf.json");
  const windows = mergeConfig(base, await readJson("src-tauri/tauri.windows.conf.json"));
  const linux = mergeConfig(base, await readJson("src-tauri/tauri.linux.conf.json"));
  const ios = mergeConfig(base, await readJson("src-tauri/tauri.ios.conf.json"));

  for (const desktop of [base, windows, linux]) {
    assert.equal(desktop.productName, identity.target.productName);
    assert.equal(desktop.identifier, identity.target.desktopIdentifier);
    assert.equal(desktop.bundle.createUpdaterArtifacts, false);
  }
  assert.equal(ios.identifier, identity.legacy.iosIdentifier);
  assert.equal(identity.activation.runtimeIdentity, true);
  assert.equal(identity.activation.dataMigration, false);
});

test("migration_allowlist_and_credentials_contract_remain_closed", async () => {
  const migration = await readJson(
    "docs/fixtures/phase-4-1c-whitelist-migration/contract.json",
  );
  const contract = await loadCloseoutContract();
  assert.equal(migration.source, "read_only");
  assert.equal(migration.target, "staged_only");
  assert.equal(migration.credentials, "never_migrated");
  assert.equal(migration.remoteHostIdentity, "deferred");
  assert.deepEqual(contract.migration.forbiddenInputs, [
    "token",
    "auth.json",
    "CODEX_HOME",
    "canonical_thread_or_rollout",
    "runtime_generation_or_pending_mutation",
  ]);
  assert.equal(contract.migration.futureTypedFields, "excluded_until_explicitly_allowlisted");
});

test("bootstrap_and_host_identity_contract_remain_fail_closed", async () => {
  const foundation = await readJson(
    "docs/fixtures/phase-4-1d-1-migration-activation-foundation/contract.json",
  );
  const startup = await readJson(
    "docs/fixtures/phase-4-1d-2-startup-cutover/contract.json",
  );
  const runtimeHandshake = await readJson(
    "docs/fixtures/phase-4-1d-3-runtime-validation-handshake/contract.json",
  );
  const recoveryRoot = await readJson(
    "docs/fixtures/phase-4-1d-4a-recovery-root-authority/contract.json",
  );
  const contract = await loadCloseoutContract();

  assert.deepEqual(foundation.identity.oldLoaderRejects, [
    "v2_active",
    "v2_retired",
    "corrupt",
    "read_blocked",
  ]);
  assert.ok(startup.normalLoadRequires.includes("committed_activation_journal"));
  assert.ok(startup.normalLoadRequires.includes("current_process_runtime_validation"));
  assert.deepEqual(startup.gatedBeforeReady, [
    "app_state",
    "global_sources",
    "daemon_listener",
    "workspace_sessions",
    "remote_auto_connect",
  ]);
  assert.equal(contract.hostIdentity.suddenPowerLossDurability, "NOT_PROVEN");
  assert.equal(contract.activation.runtimeValidatedAfter, "required_runtime_validation_success");
  assert.equal(runtimeHandshake.persistedStates.fileCommit, "target_committed");
  assert.equal(runtimeHandshake.businessAccess.targetCommitted, false);
  assert.equal(runtimeHandshake.businessAccess.currentProcessReady, true);
  assert.equal(runtimeHandshake.historicalRuntimeValidatedBypassesCurrentValidation, false);
  assert.equal(recoveryRoot.status, "pass_complete_frozen");
  assert.equal(recoveryRoot.recovery.businessReadyAfterRecovery, false);
  assert.equal(recoveryRoot.dataRoot.explicitMustBeAbsolute, true);
  assert.equal(recoveryRoot.dataRoot.relativeCwdFallback, false);
});

test("remote_targets_remain_untrusted_without_credentials", async () => {
  const contract = await loadCloseoutContract();
  assert.equal(contract.remoteTarget.tokenAfterMigration, "absent");
  assert.equal(contract.remoteTarget.trustedHostPinAfterMigration, "absent_pending_confirmation");
  assert.deepEqual(contract.remoteTarget.forbidden, [
    "automatic_connect",
    "silent_tofu_pin",
    "automatic_pin_replacement",
    "insecure_no_auth_fallback",
  ]);
});

test("failure_matrix_freezes_every_required_case", async () => {
  const contract = await loadCloseoutContract();
  const actual = new Set(contract.failureMatrix.map((entry) => entry.case));
  const required = [
    "fresh_profile",
    "valid_activated_profile",
    "legacy_migration",
    "corrupt_profile",
    "unknown_or_future_schema",
    "target_conflict",
    "path_alias_junction_reparse",
    "staging_tamper",
    "preview_or_source_change",
    "interruption_before_identity_retirement",
    "interruption_after_identity_retirement",
    "interruption_after_target_commit",
    "concurrent_activation",
    "duplicate_daemon",
    "missing_token_or_pin",
    "legacy_loader_reentry",
    "runtime_init_failure",
    "activated_profile_missing_identity",
    "daemonctl_explicit_data_dir",
    "native_non_ready_entry_gate",
    "restart_required_activation_boundary",
    "isolated_child_process_environment",
  ];
  assert.equal(contract.failureMatrix.length, required.length);
  assert.deepEqual([...actual].sort(), required.sort());
  for (const entry of contract.failureMatrix) {
    assert.equal(typeof entry.allowedAction, "string");
    assert.ok(entry.forbiddenActions.length > 0);
    assert.equal(typeof entry.resultingState, "string");
    assert.equal(typeof entry.recoveryPath, "string");
    assert.match(entry.evidenceType, /^(production_entry_test|native_temp_process|deterministic_fixture)$/);
  }
  const validActivatedProfile = contract.failureMatrix.find(
    (entry) => entry.case === "valid_activated_profile",
  );
  assert.deepEqual(validActivatedProfile, {
    case: "valid_activated_profile",
    allowedAction: "perform current-process runtime validation",
    forbiddenActions: [
      "treat persisted runtime history as current-process readiness",
      "legacy fallback",
    ],
    resultingState: "runtime_validation_required",
    recoveryPath: "strict current-process initialization then ready",
    evidenceType: "production_entry_test",
  });
  assert.deepEqual(
    contract.failureMatrix.find((entry) => entry.case === "interruption_after_target_commit"),
    {
      case: "interruption_after_target_commit",
      allowedAction:
        "classify a consistent lagging journal as recovery required and converge only the journal",
      forbiddenActions: [
        "claim runtime_validated without evidence",
        "repeat migration or identity retirement",
      ],
      resultingState: "recovery_required_then_runtime_validation_required",
      recoveryPath:
        "revalidate bindings and recovery material, converge to target_committed, then perform current-process runtime validation",
      evidenceType: "production_entry_test",
    },
  );
  assert.deepEqual(
    contract.failureMatrix.find((entry) => entry.case === "daemonctl_explicit_data_dir"),
    {
      case: "daemonctl_explicit_data_dir",
      allowedAction: "accept only a supported absolute explicit root",
      forbiddenActions: ["fallback to ambient data root", "fallback to cwd"],
      resultingState: "absolute_root_or_failure",
      recoveryPath: "provide a valid absolute activated profile root",
      evidenceType: "production_entry_test",
    },
  );
});

test("real_user_cutover_not_executed", async () => {
  const contract = await loadCloseoutContract();
  assert.deepEqual(contract.realUserOperations, {
    settingsRead: 0,
    workspaceMigration: 0,
    hostIdentityReadOrRetirement: 0,
    daemonStop: 0,
    installation: 0,
    profileCutover: 0,
  });
  assert.equal(contract.installedPackageAcceptance, "NOT_EXECUTED");
  assert.equal(contract.realUserCutover, "NOT_EXECUTED");
  assert.equal(contract.macOsInstalledAcceptance, "NOT_EXECUTED");
  assert.equal(contract.iosMigrationBuildDeviceAcceptance, "NOT_EXECUTED");
  assert.equal(contract.hostIdentity.suddenPowerLossDurability, "NOT_PROVEN");
});

test("final_current_state_authorities_are_consistent", async () => {
  const [roadmap, phase40, phase41, evidenceIndex, phase41e, readme] = await Promise.all([
    readText("CodexMonitor_四阶段开发路线图.md"),
    readText("docs/phase-4-0-truth-release-boundary.md"),
    readText("docs/phase-4-1-closeout.md"),
    readText("docs/evidence/README.md"),
    readText("docs/evidence/phase-4-1e/README.md"),
    readText("README.md"),
  ]);
  const authorities = [roadmap, phase40, phase41, evidenceIndex, phase41e, readme];
  for (const authority of authorities) {
    assert.match(authority, /Build 8/);
    assert.doesNotMatch(authority, /R01\/R03\/R04\/R06\/R11 (?:remain open|仍未收口)/);
    assert.doesNotMatch(authority, /P4\.1e (?:is )?(?:PAUSED|RESUME PENDING)/);
  }
  assert.match(roadmap, /P4\.1e[^\n]*PASS \/ COMPLETE \/ FROZEN/);
  assert.match(roadmap, /P4\.2[^\n]*NOT STARTED/);
  assert.match(phase40, /P4\.1 is \*\*PASS \/ COMPLETE \/ FROZEN\*\*/);
  assert.match(phase41, /Status: \*\*PASS \/ COMPLETE \/ FROZEN\*\*/);
  assert.match(phase41e, /Status: \*\*PASS \/ COMPLETE \/ FROZEN\*\*/);
  assert.match(readme, /P4\.1 is complete and frozen/);
});
