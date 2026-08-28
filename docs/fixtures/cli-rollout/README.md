# External Codex CLI rollout evidence fixtures

These fixtures contain sanitized records from real Codex CLI `0.147.0` runs on
2026-08-25. The runs used the default `C:\Users\DeanX\.codex` CODEX_HOME and an
isolated read-only test workspace. Identifiers, local paths, prompts, encrypted
subagent messages, installation identifiers, and account data are replaced.

The fixtures preserve only fields used for source identity, hierarchy, model,
lifecycle, Token, timestamps, and byte-tail behavior. Omitted fields are not
asserted absent from the real records.

## Captures

- `single-agent.events.jsonl`: one explicit-model CLI Turn.
- `multi-turn.events.jsonl`: the same CLI Session resumed in a new process for a
  second Turn; the original rollout file was appended rather than replaced.
- `multi-agent.files.json`: one Main plus three direct Sub-Agent rollout files.
- `app-server-rollout-pair.json`: one app-server Turn paired with its exact
  rollout records.
- `append-observations.json`: byte-length sampling and active-file access facts.
- `legacy-completed-sessions.json`: sanitized lifecycle/model/token records from
  three real pre-0.147 rollout sessions. It preserves confirmed optional-field
  omissions (`cache_write_input_tokens` and `task_complete.started_at`) that must
  not prevent the later `task_complete` from being observed.

JSONL fixtures retain record order. JSON fixtures describe relationships across
multiple files or sources. They are protocol fixtures, not examples of a future
adapter API.
