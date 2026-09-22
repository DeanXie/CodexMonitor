# P4.2a — Windows Installer Forensics / Acceptance Contract

Status: **PASS / COMPLETE / FROZEN** as the Build 8 forensic baseline. Its
three product decisions and cwd defect were resolved by P4.2b; installed-app
acceptance remains unexecuted and P4.2 remains **IN PROGRESS**.

## Evidence boundary

The production build came from clean worktree commit
`a3346513b1eab4710ff853de67ab693d06998dd4` with
`v0.7.68 · Build 8 · development`. The commands were `npm ci`,
`npm run version:check`, `npm run doctor:win`, and
`npm run tauri:build:win`. Cargo output was redirected by repository storage
governance to
`F:\AI\_cargo-target\CodexMonitor\agents\phase-4-2a-windows-installer-forensics-0d02e372144e`.
The main checkout's four user-local files did not participate.

No installer or uninstaller was executed. No installed executable was launched,
no registry or shortcut was changed, no real AppData profile was written, and
no real migration was attempted. Generated WiX/NSIS source, MSI database
tables, PE metadata, hashes, and a temporary read-only MSI extraction are the
authority for this report. Anything requiring Windows Shell execution is
marked `NOT_EXECUTED` or `NOT_PROVEN_BY_STATIC_INSPECTION`.

The available build chain did not include a separate NSIS archive extractor.
Its exact `File` directives and metadata are proven by generated source, but an
independent extraction of the compressed NSIS payload is
`NOT_PROVEN_BY_STATIC_INSPECTION`; case B therefore combines generated-source
authority with the independently extracted MSI payload.

## Artifacts

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `CodexMonitor DeanX_0.7.68_x64_en-US.msi` | 16,736,256 | `DD7CABF9BCD1E777E6E7701A9C123FBB43207F7FE7B005B85884D6B8C0067D8E` |
| `CodexMonitor DeanX_0.7.68_x64-setup.exe` | 12,617,853 | `C3B671152DA9ED6E0F896FF53AC207EF7E1AE3AE87536726D43AD093E334D76B` |
| release `codex-monitor.exe` | 32,592,384 | `84CAF7F38082F2E971DE929B38BC32A21343E3C8347E6AA172056217FA492741` |
| release `codex_monitor_daemon.exe` | 11,001,856 | `20E2C00306A60B17C8B979D2A55E3B68DC767C3BD94E83F9242D54AFD23EADBC` |
| release `codex_monitor_daemonctl.exe` | 1,574,912 | `029A7BB17F9CFC918B8C2400431DD46E59BFE8D11B221E921468CAAAA3C7E959` |

The release main executable is patched between MSI and NSIS bundling, so the
read-only MSI-extracted main payload has a different SHA-256
(`BE6D62259DE0069F050CC514698930D46439FFB4F60DC0E6BCB1D253D834FFFA`)
while retaining the same size and `0.7.68.0` file version. The MSI-extracted
daemon hash exactly matches the release daemon hash above.

## Effective configuration

The merged base/Windows configuration resolves to:

- product name `CodexMonitor DeanX`;
- identifier `io.github.deanxie.codexmonitor`;
- version `0.7.68`;
- bundle target `all`, yielding MSI and NSIS on Windows;
- executable `codex-monitor.exe` and `icon.ico`;
- `createUpdaterArtifacts = false`, no updater plugin configuration, endpoint,
  public key, permission, or release-manifest chain;
- WebView2 `downloadBootstrapper`, a Tauri schema default confirmed in both
  generated installers rather than an explicit project setting.

`release-identity.json` is the stable DeanX identity authority. The generated
MSI has stable UpgradeCode `{EF5D62BE-6AAC-54C9-BDBA-8DE1EE80B86A}` and a
per-build ProductCode `{26CD2216-DD74-4244-AFA7-43598EF57FF6}`. The release
workflow uploads ordinary MSI/NSIS artifacts and release notes; it does not
produce `latest.json` or updater signatures.

## Payload truth

Both installers place `codex-monitor.exe` and `codex_monitor_daemon.exe` in
the same installation directory. `daemon_binary.rs` checks the current
executable directory before platform fallbacks, so installed daemon discovery
without `CODEX_MONITOR_DAEMON_PATH` is **STATIC PATH CONTRACT PASS**. The
authenticated launch/readiness path remains installed E2E evidence.

`codex_monitor_daemonctl.exe` is built but is absent from both installer
payloads. It is not required for the ordinary GUI baseline. Keep it out of the
daily-use installer until a later headless/service-management requirement
explicitly makes it a user-facing tool.

## MSI and NSIS comparison

| Property | MSI | NSIS |
| --- | --- | --- |
| Scope | per-machine | per-user |
| Elevation | administrator required | no installer elevation declared |
| Default directory | `Program Files\CodexMonitor DeanX` | `%LOCALAPPDATA%\CodexMonitor DeanX` |
| Main + daemon | yes, same directory | yes, same directory |
| Start Menu | created | created |
| Desktop shortcut | unconditionally included | finish-page option, checked by default; silent/passive always creates |
| User opt-out | no | yes for interactive install |
| Uninstall registration | per-machine ARP | per-user ARP |
| Default data retention | app data untouched | delete-data checkbox is initially unchecked |
| WebView2 absent | PowerShell download/bootstrap | NSIS download/bootstrap |

The approved shortcut contract is Start Menu by default and Desktop checked by
default with user opt-out. NSIS meets that contract only for interactive
installation. MSI does not. The two formats also disagree on privilege and
scope; they are not interchangeable acceptance authorities.

## Windows version and upgrade semantics

Windows-visible installer and uninstall versions are `0.7.68`; PE resources
use `0.7.68.0`. Build 8 is absent. NSIS explicitly classifies a second package
with the same SemVer as same-version reinstall/maintenance. WiX uses a stable
UpgradeCode and a new ProductCode but retains the same three-field
ProductVersion. The exact same-SemVer MSI transition must be observed in a
disposable installed E2E; static evidence does not prove whether the desired
upgrade succeeds, enters maintenance, or conflicts.

Therefore Build 8 to Build 9 with unchanged SemVer is not a defined Windows
upgrade. The recommended release rule is: every published Windows installer
advances SemVer, while monotonic Build remains the repository authority and is
never reset. Mapping Build into an unsupported fourth MSI ProductVersion field
is not proposed.

## Runtime dependency audit

| Dependency/path | Classification | Static conclusion |
| --- | --- | --- |
| Node/npm, Rust/Cargo, CMake, LLVM/libclang | build-only | no installed core dependency |
| WebView2 Runtime | required installed runtime | current installer downloads when missing |
| Codex CLI | optional feature dependency | workspace app-server features require it; shell startup does not statically invoke it |
| Git | optional feature dependency | Git features require it |
| `gh` | optional feature dependency | GitHub features require it |
| PowerShell | MSI bootstrap dependency | generated MSI uses it to download WebView2 |
| `CODEX_MONITOR_DAEMON_PATH` | development/operator override | installed discovery does not need it |
| repo, `target`, `dist`, `.worktrees`, local BAT files | forbidden runtime dependency | no main/daemon launch lookup found |
| process working directory | unexpected fallback | dictation model storage falls back to cwd when Tauri app-data resolution fails |

App/daemon/daemonctl activated roots are absolute and fail closed; the desktop
App uses Tauri `app_data_dir`, daemon/daemonctl use the shared target-root
resolver, and daemon discovery is executable-relative. `CODEX_HOME` resolves
from explicit environment or the user home and Codex itself is PATH/config
resolved only when a WorkspaceSession is requested. Dynamic proof that the
installed shell starts without optional tools remains case K.

The dictation cwd fallback is the only production installed-path defect found
by static audit. It does not prove first launch fails, but it violates the
no-cwd-fallback safety contract when app-data resolution fails and should be a
bounded packaging correction before installed acceptance.

## Network and telemetry

Updater code paths and Sentry initialization/dependencies remain disabled.
There is no updater endpoint, updater public key, `latest.json`, or updater
artifact chain. However, both installers perform an automatic Microsoft
WebView2 network bootstrap when the runtime is absent. Installer network
silence is therefore **false**. Installed runtime network silence is
**STATIC CONTRACT VERIFIED / DYNAMIC ACCEPTANCE NOT_EXECUTED**, excluding
normal user-requested Codex/GitHub/remote-backend traffic.

## Resolved decisions

P4.2b selected NSIS per-user, offlineInstaller, and SemVer-based Windows
upgrade identity, and removed the dictation cwd fallback. MSI remains a
non-canonical compatibility artifact. This report remains the unmodified Build
8 observation authority; the corrected Build 9 authority is
`docs/phase-4-2b-windows-packaging-corrections.md`.

## Disposable installed E2E recommendation

Use a Windows Sandbox or disposable VM snapshot, not environment-variable
redirection in the real DeanX account. Registry, Shell known folders,
WebView2, shortcuts, elevation, and Tauri app-data resolution must be real
inside the guest. Feed the selected installer and sanitized fixtures through a
mapped read-only host folder; capture hashes, MSI/NSIS logs, process trees,
shortcuts, installed files, ARP entries, app-data tree, loopback ownership, and
network traces; export only non-sensitive evidence and revert the snapshot.

Automation should drive build artifact verification and post-install evidence
collection. Installation, first-launch UI, desktop opt-out, profile bootstrap,
and destructive uninstall choices should remain bounded guest actions. The
machine-readable A–Q matrix is
`docs/fixtures/phase-4-2a-windows-installer-forensics/contract.json`.

## Slice recommendation

- **P4.2a:** this forensics/design record; frozen.
- **P4.2b:** canonical packaging corrections; frozen.
- **P4.2c:** disposable clean install, first launch, bootstrap, daemon, dev-tool
  independence, and network evidence.
- **P4.2d:** uninstall/reinstall/upgrade/shortcut and retention matrix.
- **P4.2e:** aggregate Windows daily-use closeout.

P4.2c is the next slice and must not treat static packaging evidence as an
installed-app acceptance result.
