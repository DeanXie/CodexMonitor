import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const fixtureUrl = new URL(
  "../docs/fixtures/phase-4-2a-windows-installer-forensics/contract.json",
  import.meta.url,
);

async function loadFixture() {
  return JSON.parse(await readFile(fixtureUrl, "utf8"));
}

test("freezes the Build 8 installer artifacts and payload truth", async () => {
  const fixture = await loadFixture();

  assert.equal(fixture.sourceCommit, "a3346513b1eab4710ff853de67ab693d06998dd4");
  assert.deepEqual(fixture.versionAuthority, {
    version: "0.7.68",
    build: 8,
    status: "development",
  });
  assert.equal(
    fixture.artifacts.msi.sha256,
    "DD7CABF9BCD1E777E6E7701A9C123FBB43207F7FE7B005B85884D6B8C0067D8E",
  );
  assert.equal(
    fixture.artifacts.nsis.sha256,
    "C3B671152DA9ED6E0F896FF53AC207EF7E1AE3AE87536726D43AD093E334D76B",
  );
  assert.equal(
    fixture.artifacts.mainExecutable.sha256,
    "84CAF7F38082F2E971DE929B38BC32A21343E3C8347E6AA172056217FA492741",
  );
  assert.equal(fixture.payload.daemon.packaged, true);
  assert.equal(fixture.payload.daemon.sameDirectoryAsMainExecutable, true);
  assert.equal(fixture.payload.daemonctl.packaged, false);
});

test("keeps installer differences and unresolved decisions explicit", async () => {
  const fixture = await loadFixture();

  assert.equal(fixture.installerContracts.msi.scope, "per_machine");
  assert.equal(fixture.installerContracts.nsis.scope, "per_user");
  assert.equal(fixture.installerContracts.msi.desktopShortcut.userCanOptOut, false);
  assert.equal(fixture.installerContracts.nsis.desktopShortcut.defaultChecked, true);
  assert.equal(fixture.installerContracts.nsis.desktopShortcut.userCanOptOut, true);
  assert.equal(fixture.webView2.mode, "download_bootstrapper");
  assert.equal(fixture.webView2.automaticNetworkBootstrap, true);
  assert.equal(fixture.windowsVersioning.buildNumberProjected, false);
  assert.equal(fixture.contractStatus, "DRAFT_DECISION_REQUIRED");
  assert.ok(fixture.decisionRequired.length >= 3);
});

test("separates completed static cases from unexecuted installed cases", async () => {
  const fixture = await loadFixture();
  const expectedIds = "ABCDEFGHIJKLMNOPQ".split("");

  assert.deepEqual(
    fixture.acceptanceCases.map((entry) => entry.id),
    expectedIds,
  );
  for (const entry of fixture.acceptanceCases) {
    assert.ok(entry.precondition);
    assert.ok(entry.action);
    assert.ok(entry.expectedResult);
    assert.ok(entry.forbiddenResult);
    assert.ok(entry.evidenceType);
    assert.ok(entry.destructiveScope);
    assert.ok(entry.cleanup);
  }
  assert.equal(fixture.acceptanceCases[0].status, "PASS_SOURCE_BUILD");
  assert.equal(fixture.acceptanceCases[1].status, "PASS_STATIC_INSPECTION");
  for (const entry of fixture.acceptanceCases.slice(2)) {
    assert.equal(entry.status, "NOT_EXECUTED");
  }
});

test("preserves the read-only forensics boundary", async () => {
  const fixture = await loadFixture();

  assert.equal(fixture.operations.installerExecuted, false);
  assert.equal(fixture.operations.installedAppLaunched, false);
  assert.equal(fixture.operations.uninstallerExecuted, false);
  assert.equal(fixture.operations.registryModified, false);
  assert.equal(fixture.operations.realProfileWritten, false);
  assert.equal(fixture.operations.phase42bStarted, false);
});
