# E2E acceptance evidence

This directory stores durable, sanitized summaries for formal E2E gates. It
does not replace the rotating diagnostic journal and must not contain prompts,
reasoning, message content, account data, or raw diagnostic records.

Create a summary from a reviewed JSON candidate with:

```powershell
node scripts/e2e-evidence-summary.mjs candidate.json docs/evidence/phase-x-y-scenario.json
```

The writer accepts only the documented identity, hierarchy, source lane,
observed model, cumulative Token, lifecycle, freshness, lag, cursor, and result
fields. Files are formatted as stable JSON and written through a temporary file
before rename. Preserve the 16 MiB diagnostic journal rotation independently.

Do not reconstruct missing protocol evidence. If an earlier raw journal has
already rotated out, retain the human acceptance result in project status docs
and start durable summaries with the next gate that has verifiable identities.
