# App-server realtime evidence fixtures

These fixtures contain sanitized records copied from the CodexMonitor Debug Panel. They preserve the observed app-server payload shape and event ordering while replacing local identifiers, paths, prompts, and developer instructions.

## Captures

- `single-agent.events.json`: one newly created top-level agent from `thread/start` through `turn/completed`.
- `multi-agent-start.events.json`: one existing main agent spawning three direct children; the Debug Panel retained all 142 records in this capture.
- `multi-agent-completion.events.json`: the completion window from the first three-child run; the Debug Panel retained its maximum 200 records, so early events are explicitly marked as evicted.

Each fixture contains only records relevant to identity, hierarchy, model observation, lifecycle, token usage, and timestamps. Unrelated streaming deltas, MCP startup notifications, rate-limit notifications, stderr, and command output are excluded.

The fixtures do not add absent protocol fields. In particular, neither multi-agent capture contains a child `thread/started`, child `thread/settings/updated`, or `model/rerouted` notification.

## Writer-admission observation compatibility

`writer-admission-observation/` freezes the Phase 3.5.2b.5 sanitized request,
response/error, generation, canonical Thread identity, attempt provenance, and
normalized observation contract. `exact-resume-accepted.json` and
`active-writer-blocked.json` derive from the frozen Phase 3.5.2a A3 and A2
evidence; the remaining files freeze the tested b.2-b.4 contract boundaries.

`protocol-provenance.json` pins the bundled Codex executable version/hash and
the checked upstream Codex commit. It also records that CodexMonitor's
`thread_live_unsubscribe` is synthetic and that upstream `thread/unsubscribe`
is subscription lifecycle evidence, not writer-release acknowledgement.

These fixtures deliberately contain no token, authorization secret, writer or
lease identity, Remote-client ownership, or global `FREE`, `AVAILABLE`, or
`RELEASED` state.

## Upstream thread unsubscribe

`thread-unsubscribe/` freezes the bundled `thread/unsubscribe` request shape
and the three normalized response statuses used by Phase 3.5.2c.3. The
fixtures are protocol evidence for app-server connection subscription state;
they do not acknowledge runtime unload or writer release. The provenance file
also freezes the no-retry and writer-observation non-transition boundaries.
