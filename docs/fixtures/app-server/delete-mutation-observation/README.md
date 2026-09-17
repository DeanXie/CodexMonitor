# Delete mutation observation fixtures

These sanitized fixtures freeze Phase 3.5.3c against fake app-server authority
only. They contain one synthetic full Thread UUID, no user paths, no token, and
no real deletion target.

`thread/delete` success with an exact empty result and an exact current-
generation `thread/deleted` notification are direct deletion evidence. Active-
writer rejection is typed. Lost responses and session end after possible
dispatch remain unresolved and cannot create a tombstone. No fixture defines a
Remote-client identity, deletion owner, lease, retry, replay, or takeover.
