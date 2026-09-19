# P4.1d-2 — Startup Cutover / RemoteHostIdentity Safe Migration

**Goal:** Wire the frozen P4.1d-1 activation foundation into the App, daemon, and daemonctl startup boundaries without reading or migrating real user data during verification.

**Architecture:** Add one shared startup authority that resolves the approved desktop roots, classifies bootstrap state, and opens only fully activated profiles. App, daemon, and daemonctl remain thin adapters. The App exposes a minimal bootstrap boundary while business initialization is absent; daemon and daemonctl fail closed before listening or starting a process. Existing activation primitives remain the only fresh/migration commit implementation.

## Tasks

1. Add RED tests for strict root resolution, activation-only versus normal-load decisions, corrupt/future/conflicting profiles, no identity generation during inspection, and App/daemon/daemonctl parity.
2. Add RED release-identity tests for the approved DeanX desktop identity while keeping iOS migration pending.
3. Implement the shared startup authority and strict activated-profile loader using P4.1d-1 inspection and v2 identity validation.
4. Gate App setup before `AppState`, global sources, daemon autostart, sessions, and remote initialization; expose only minimal bootstrap status/activation commands until restart into a valid profile.
5. Gate daemon before state construction/listener bind and daemonctl before command preview/start/status/stop can use a supplied or default root.
6. Route App workspace data-root consumers through `AppState.storage_path` instead of a second resolver.
7. Switch only the approved desktop product name/identifier and daemonctl/default-root authority; leave the iOS identifier and real user data untouched.
8. Run focused tests, P4.1a/b/c/d-1 regression, Rust all-targets/check/fmt, typecheck, frontend tests, production build, version check, and diff check.
9. Bump the canonical build once to Build 4, explicitly stage scoped files, commit, ff-only merge, and push.

## Safety gates

- Tests use only temporary roots, fake identities/configuration, and controlled child processes.
- No access to real AppData profiles, credentials, canonical Thread data, or live daemons.
- No real identity retirement or real cutover.
- The current main-worktree `.gitignore`, `src-tauri/src/main.rs`, and local BAT changes are never staged, restored, stashed, or reset.
