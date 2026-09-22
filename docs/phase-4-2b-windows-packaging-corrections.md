# P4.2b — Canonical Windows Packaging Corrections

Status: **PASS / COMPLETE / FROZEN** at
`v0.7.68 · Build 9 · development`. P4.2 remains **IN PROGRESS** and P4.2c is
**NOT STARTED**.

## Canonical package contract

- NSIS `currentUser` is the canonical Windows daily-use installer.
- Start Menu is created by default. The Desktop shortcut is selected by
  default and can be declined in the interactive installer.
- MSI remains a non-canonical compatibility artifact; it is not accepted as
  equivalent to the NSIS scope, privilege, or shortcut contract.
- Windows installer upgrade identity is SemVer. Monotonic Build remains the
  repository revision authority and does not create a Windows upgrade while
  SemVer is unchanged.
- `codex_monitor_daemon.exe` is bundled next to `codex-monitor.exe` and is
  resolved executable-relative. `codex_monitor_daemonctl.exe` is not required
  or bundled for the GUI baseline.

## Offline WebView2 boundary

The merged Windows config fixes `webviewInstallMode` to `offlineInstaller` and
pins the audited NSIS template at `src-tauri/windows/nsis/installer.nsi`. The
template is derived from Tauri CLI 2.10.1 / tauri-bundler 2.8.1 commit
`9b17a7aeae9a83222ffe829aa4e2d8a5ba6bed8c`; bootstrapper modes and their
online URL are removed. The production build downloads Microsoft's official
offline runtime while building and embeds it. The generated installer does not
download WebView2 at install time.

Static evidence confirms the generated NSIS source contains the offline
payload `File` instruction and contains no `NSISdl::download`,
`downloadBootstrapper`, or bootstrapper URL. This does not replace disposable
installed network observation in P4.2c.

## Installed-path correction

Dictation model-root resolution now fails closed when Tauri cannot resolve
`app_data_dir`; it no longer uses the process current working directory. A
test places a valid-looking model under cwd and proves resolution still fails.
The successful path remains `<app_data_dir>/models/whisper`.

## Artifact evidence

The production build used the repository wrapper and external Cargo target.
It generated the following without executing either installer:

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| NSIS `CodexMonitor DeanX_0.7.68_x64-setup.exe` | 228,436,251 | `F9A4B935A923ED8ADFA22550F44FF0A64E4A795EBCA562FDDE4E077FE2304B81` |
| MSI `CodexMonitor DeanX_0.7.68_x64_en-US.msi` | 230,002,688 | `D3AB9F6B92FB628971B681849110EF10868FA6480AFB53F9C767D35CC6945BCD` |
| `codex-monitor.exe` | 32,592,384 | `24415897A57425E16FFB20030AF534EB774CB2ADAA788944CB8F13D64869D3C6` |
| `codex_monitor_daemon.exe` | 11,001,856 | `880E5CA5E6E90F9BCC4C2DE542EF35AFF4DE1D25FAF5BB65050F01B6182D6687` |

The larger installers are expected because they embed the offline WebView2
runtime. Updater artifact generation and Sentry remain disabled.

## Evidence boundary

No installer, uninstaller, or installed executable was run. No registry,
shortcut, real profile, HostIdentity, or real migration state was changed.
Install, first launch, shortcut behavior, daemon readiness from an installed
location, installer-time network silence, uninstall, reinstall, and upgrade
remain P4.2c/P4.2d disposable-environment acceptance work.

The machine-readable authority is
`docs/fixtures/phase-4-2b-windows-packaging-corrections/contract.json`.
