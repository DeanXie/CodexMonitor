# P4.2 Windows Installed Daily-use Baseline Implementation / Acceptance Plan

> **Status:** draft; blocked on installer scope, WebView2 delivery, and Windows-visible version decisions. Do not execute until explicitly authorized.

**Goal:** Produce one deterministic Windows daily-use installer contract and prove it in a disposable Windows environment without touching the real DeanX profile.

**Architecture:** Select one canonical package authority, make the smallest packaging/path corrections, then execute the machine-readable A–Q acceptance matrix in a Windows Sandbox or disposable VM. Static evidence, guest installed evidence, and any later real-user evidence remain separate.

**Spec:** `docs/superpowers/specs/2026-09-22-p4-2-windows-installed-baseline-design.md`

---

## Task 1: Resolve product decisions

- Choose canonical NSIS per-user, MSI per-machine, or full parity.
- Choose offline WebView2, embedded bootstrapper, or explicit prerequisite.
- Choose the rule that advances Windows-visible version for every published installer.
- Record the choices in the existing P4.2 spec/fixture; do not invent a parallel state authority.

Completion: the fixture is no longer `DRAFT_DECISION_REQUIRED` and contains no ambiguous package authority.

## Task 2: Minimal packaging corrections (conditional P4.2b)

- Add RED tests for the selected package contract.
- Correct only WebView2 delivery, selected shortcut/scope behavior, Windows version/release gates, and the dictation app-data cwd fallback.
- Preserve updater/Sentry disablement, DeanX identity, P4.1 migration gates, and daemon colocation.
- Keep daemonctl excluded unless a separately approved headless requirement needs it.

Completion: clean production build and static inspection satisfy the selected contract; no install is performed in this task.

## Task 3: Prepare disposable guest automation

- Create a clean Windows Sandbox/VM image without repo or developer tools.
- Map artifacts and sanitized fixtures read-only.
- Add guest evidence scripts for files, hashes, shortcuts, ARP entries, registry scope, data roots, process trees, listener ownership, and network capture.
- Make cleanup snapshot-based and prove no host AppData/registry path is targeted.

Completion: dry-run evidence collection against synthetic guest files only; no real profile access.

## Task 4: Execute A–L installed baseline cases

- Run clean build/payload verification and guest install.
- Run first launch, fresh activation, sanitized legacy migration, restart,
  shortcut checks, daemon discovery, dev-tool independence, and network silence.
- Record source/config, installer-static, and installed-E2E evidence separately.

Completion: A–L pass, including Desktop opt-out and zero updater/Sentry traffic; WebView2 behavior matches the selected contract.

## Task 5: Execute M–Q lifecycle cases

- Run uninstall, reinstall, versioned upgrade, data retention, and corrupt-profile fail-closed cases in fresh guest snapshots.
- Use two packages with a deliberately approved Windows-visible version progression for upgrade.

Completion: M–Q pass with one product identity, bounded data deletion, and zero fallback/business initialization on corrupt profiles.

## Task 6: Close P4.2

- Run fresh focused/full regressions, version/config checks, production build,
  artifact inspection, and diff checks.
- Update the existing roadmap/evidence index and aggregate P4.2 authority.
- Explicitly stage scoped files, commit, ff-only merge, push, and verify refs.

Completion: P4.2a–e evidence is coherent, all applicable A–Q cases pass, and P4.2 is `PASS / COMPLETE / FROZEN`. Stop before P4.3 unless separately authorized.
