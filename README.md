# Why This Way

Why This Way gives coding agents the decisions and invariants that govern a
repository before they change it.

```text
WTW: what governs this change, and why?
RTW: how is this implemented correctly here?
NYA: which corrected failure must not recur?
AVP: how is the behavior proved?
NWC: which previously blocked action can proceed now?
```

WTW is local-first, Git-native, provider-independent, and usable as a CLI,
Rust library, or stdio MCP server.

## Install

From source:

```bash
cargo install --path .
```

Initialize a Git repository:

```bash
wtw init
```

This creates `.agent-first/wtw`, installs the managed agent instructions, and
creates an ignored local judge configuration using Codex. Edit
`.agent-first/wtw/config.local.toml` to use any command that reads one prompt
from stdin and writes the requested JSON to stdout.

## Daily protocol

Retrieve governing context before editing:

```bash
wtw explain \
  --task "change payment capture" \
  --path src/payments/capture.rs
```

Collect after an accepted ADR, plan, explicit decision, or completed work:

```bash
wtw collect \
  --task "adopt idempotent capture" \
  --source docs/adr/0042-idempotent-capture.md
```

There is intentionally no `wtw add`.

Audit the final diff:

```bash
wtw guard --task "implemented idempotent capture"
```

Inspect repository governance:

```bash
wtw show --id payments.capture-at-most-once
wtw health
wtw export > wtw-graph.json
```

With federated graph exports from the other foundations:

```bash
wtw health --suite --graph avp-graph.json --graph rtw-graph.json
```

Suite mode fails while any active invariant lacks an inbound active AVP proof.

## Records

A decision:

```toml
schema = 1
id = "direct-appdb"
kind = "decision"
status = "active"
title = "Use AppDb directly"
statement = "Handlers access AppDb directly"
rationale = "Repository abstractions obscure slice ownership"
scopes = ["src/backend/**"]
evidence = [
  "repository abstractions obscure slice ownership",
  "repository-per-entity alternative was explicitly rejected",
]

[[alternatives]]
statement = "Repository per entity"
rejected_because = "It obscures slice ownership"

[authority]
kind = "adr"
source = "docs/architecture.md"
quote = "We choose direct AppDb access"

[[links]]
rel = "upholds"
to = "wtw://invariant/module-write-ownership"
basis = "Direct access keeps the writing module visible"
```

An invariant replaces alternatives with a concrete `violation`.

## MCP

```bash
wtw mcp
```

Tools:

- `wtw_collect`
- `wtw_explain`
- `wtw_guard`
- `wtw_show`
- `wtw_health`
- `wtw_export`

All call the same core operations as the CLI.

## Verification

```bash
cargo xtask verify
```

The gate runs formatting, Clippy, the production line budget, all tests, and
95% line coverage.

Licensed under MIT.
