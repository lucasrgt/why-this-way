<!-- wtw:instructions:start -->
## Why This Way

This repository uses Why This Way (`wtw`) to preserve the decisions and
invariants that govern future changes.

1. At task start, run `wtw explain --task "<goal>" --path <expected-path>`.
2. Treat every returned active invariant as a constraint and every returned
   decision as governing context.
3. Rerun `wtw explain` when scope changes or context is compacted.
4. The host runs `wtw collect` after accepted plans, ADRs, explicit decisions,
   and completed work. Agents never add records manually.
5. Before completion, run `wtw guard --task "<completed task>"`. Exit code 1
   means the governing graph is unhealthy and exit code 2 is an incomplete
   check.

Tests and documentation do not replace `wtw guard`.
<!-- wtw:instructions:end -->
