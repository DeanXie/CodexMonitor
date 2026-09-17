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

`thread-lifecycle-reconciliation/` contains sanitized bundled notification
shapes used by Phase 3.5.2c.4. `thread/closed` and
`thread/status/changed(status.type = notLoaded)` are runtime-availability
evidence only; the fixtures do not encode unsubscribe success, writer release,
ownership, or a lease. `protocol-provenance.json` binds those shapes to the
bundled Codex version and freezes their non-transition contract.

## Thread lifecycle observation compatibility

`thread-lifecycle-observation/` is the Phase 3.5.2c.5 normalized contract
fixture set. It freezes all subscription and runtime state spellings, synthetic
local detach versus upstream unsubscribe, the three upstream response mappings,
ambiguous no-retry outcomes, delayed `thread/closed`, connection and
WorkspaceSession generation reset, reconnect isolation, and multi-subscriber
connection isolation.

The runtime and close fixtures include complete sanitized
`thread/status/changed:notLoaded` and `thread/closed` messages so compatibility
tests drive the production ingestion function. The upstream outcome fixture
also drives the real shared App core and daemon RPC boundary for all three
successful response statuses, including one-dispatch/zero-retry assertions.

Every scenario carries a WorkspaceSession generation, app-server connection
generation, `CodexThreadKey`, timestamps, attempt/request evidence when
applicable, and separate runtime evidence. The fixtures contain no credentials,
Remote-client subscription owner, writer owner, lease, or global writer
freedom/release state.

## Approval request observation

`approval-request-observation/` freezes sanitized bundled message shapes for
command-execution, file-change, and permissions approval requests, exact
`serverRequest/resolved`, exact request-correlated `item/completed`, and
auto-review lifecycle annotation. It contains no command text, path, token,
decision result, Remote-client identity, owner, or lease evidence.

## Approval decision provenance

`approval-decision-provenance/` freezes the Phase 3.5.3b bundled response
families, exact generation/approval identity scope, single-admitted-attempt
gate, write-boundary classification, and zero retry/replay contract. The
fixtures are deterministic fake-authority inputs only; they do not answer a
real approval request or claim that a dispatched response was applied.

## Delete mutation observation

`delete-mutation-observation/` freezes the Phase 3.5.3c exact full-Thread-ID
request, bundled empty-success response, active-writer rejection,
current-generation `thread/deleted` confirmation, post-dispatch response-loss,
and session-ended unresolved contracts. The fixtures contain sanitized UUIDs
and no real Thread, rollout, A3 target, Remote-client identity, delete owner, or
lease. They are consumed only by fake app-server and deterministic session
tests; retry and replay remain zero.

## Delete mutation isolation

`delete-mutation-isolation/` freezes the Phase 3.5.3d same-key active gate,
different-key concurrency, stale-generation rejection, pre/post-write transport
loss, direct-evidence precedence, session-end ambiguity, new explicit intent
after unknown, and confirmed-tombstone projection isolation. The fixtures use
only synthetic UUIDs and deterministic fake authority. They prove local
single-dispatch behavior, not upstream concurrent-delete ordering.
