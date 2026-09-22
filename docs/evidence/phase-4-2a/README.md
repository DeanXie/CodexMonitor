# P4.2a Evidence

Status: **PASS / COMPLETE / FROZEN** as the Build 8 forensic baseline. P4.2b
resolved its decisions; no installed-app E2E has been executed.

Evidence authority:

- source commit `a3346513b1eab4710ff853de67ab693d06998dd4`;
- `v0.7.68 · Build 8 · development`;
- clean-worktree production build through repository storage wrappers;
- generated WiX `main.wxs` and NSIS `installer.nsi`;
- read-only Windows Installer database queries;
- read-only MSI payload extraction with Tauri's existing WiX `dark.exe`;
- PE file metadata and SHA-256 hashes;
- local Tauri 2.10 configuration schema/defaults;
- source/config/dependency audit.

The resulting artifact hashes, configuration, payload truth, decision gates,
and A–Q acceptance cases are frozen as observed evidence in
`../../fixtures/phase-4-2a-windows-installer-forensics/contract.json` and
explained in `../../phase-4-2a-windows-installer-forensics.md`.

At the time of this capture, no MSI/NSIS install, silent install, uninstall,
installed-app launch, registry write, real shortcut, real AppData write, real
profile migration, or P4.2b implementation had occurred. Temporary extraction
contained package payload only.
