# App-server realtime evidence fixtures

These fixtures contain sanitized records copied from the CodexMonitor Debug Panel. They preserve the observed app-server payload shape and event ordering while replacing local identifiers, paths, prompts, and developer instructions.

## Captures

- `single-agent.events.json`: one newly created top-level agent from `thread/start` through `turn/completed`.
- `multi-agent-start.events.json`: one existing main agent spawning three direct children; the Debug Panel retained all 142 records in this capture.
- `multi-agent-completion.events.json`: the completion window from the first three-child run; the Debug Panel retained its maximum 200 records, so early events are explicitly marked as evicted.

Each fixture contains only records relevant to identity, hierarchy, model observation, lifecycle, token usage, and timestamps. Unrelated streaming deltas, MCP startup notifications, rate-limit notifications, stderr, and command output are excluded.

The fixtures do not add absent protocol fields. In particular, neither multi-agent capture contains a child `thread/started`, child `thread/settings/updated`, or `model/rerouted` notification.
