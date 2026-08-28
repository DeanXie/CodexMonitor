# Desktop rollout fixtures

These fixtures are sanitized protocol slices captured from a real Codex Desktop
Main Agent with three direct Sub-Agents on 2026-08-27.

They retain only fields needed to test identity, parent/child relationships,
turn lifecycle, observed model, cumulative token snapshots, and timestamps.
They do not contain prompts, reasoning, agent messages, credentials, or private
diagnostic payloads.

- `desktop-main-multiturn.jsonl` proves that a Desktop Main Thread uses the
  standard rollout record families and appends multiple Turns to one file.
- `desktop-subagent-compacted-prefix.jsonl` proves that a Sub-Agent spawned from
  a long/compacted Thread can contain a file-owner `session_meta`, followed by
  replayed parent history including another `session_meta`, before the child
  Turn begins. The first `session_meta` is the source-file owner; replayed
  history must not replace it.
- `evidence-summary.json` contains only aggregate, allow-listed forensic facts.

The IDs, timestamps, paths, and token values are synthetic but preserve the
observed relationships and ordering.
