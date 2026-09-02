# Phase 3.2.5 Project / Workspace contract fixtures

`contract-scenarios.json` is the sanitized executable fixture set for Gates A–L.
It contains only synthetic Thread/Turn/Project IDs and root locators required to
exercise the frozen Phase 3.2 contracts. It contains no prompt, response,
reasoning, credential, account, token, cookie, or Desktop private-state content.

The Rust acceptance suite loads these records and invokes the existing
`workspace_interop_core` resolver, scoped relation store, runtime reconciler,
and Desktop Project projection resolver. The fixture never implements a second
resolver and never treats Project roots as Thread assignment evidence.

Run the suite with:

```powershell
cd src-tauri
cargo test --lib phase_3_2_5_contract -- --nocapture
```

Real-environment results are stored separately under
`docs/evidence/phase-3-2-5/`; missing observations remain explicit and are not
backfilled from these synthetic fixtures.
