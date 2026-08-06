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

This creates `.wtw`, installs the managed agent instructions, and
creates an ignored local judge configuration using Codex. Edit
`.wtw/config.local.toml` to use any command that reads one prompt
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

Large diffs are partitioned into bounded, path-aware envelopes. Every envelope
receives the same isolated two-pass review, so repository size does not require
weakening the gate or exceeding the judge's context limit.

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

## Measured evidence

WTW publishes paired agent evidence and deterministic large-corpus stress
results. The two measurements answer different questions and are not combined
into one prevention-rate claim.

### Paired agent benchmark

Five focused coding tasks were each run once without WTW and once with one
relevant, versioned WTW record. Arm order was randomized and the resulting code
was classified by deterministic evaluators outside the agent.

| Measurement | Result |
| --- | ---: |
| Proven preventions | 1 |
| Passing ties | 4 |
| Incomplete baseline arms | 0 |
| Regressions | 0 |
| WTW arms passing | 5 of 5 |
| `wtw explain` observed | 5 of 5 |
| `wtw guard` observed | 5 of 5 |

A prevention is counted only when the baseline contradicts the accepted
decision and the WTW arm passes. A baseline that already passes is a passing
tie, not a prevention.

### Large-corpus stress

| Records | Exact retrieval | Ranked first | False positive retrievals | Guard | Explain p95 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1,024 | 64 of 64 | 64 of 64 | 0 | pass | 209.81 ms |
| 10,000 | 64 of 64 | 64 of 64 | 0 | pass | 1,001.49 ms |

Both corpora exported every record, passed graph health, kept retrieval bounded
to the requested limit, distinguished a contradicting diff from a compliant
one, and rejected corrupt storage. Timings are from one Windows development
machine and are not portable performance guarantees. The first cold export of
the 10,000-file corpus took 53.96 seconds; normal explain calls remained close
to one second at p95.

The repositories and stress corpus are realistic but synthetic. The paired
sample is deliberately too small for a universal prevention-rate claim. Read
the [protocol, raw artifacts, and machine-readable results](benchmarks/README.md).

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


## Prime Agent

The optional package at `integrations/prime-agent` provides bounded automatic
`explain`, explicit `/wtw` checks, and a conditional model skill for
repositories already adopted through `.wtw/SKILL.md`. A root
`csm.toml` has precedence and suppresses this standalone adapter to prevent
duplicate retrieval and verification. It never initializes the repository or
writes semantic records. See the [Prime Agent guide](docs/prime-agent.md).

## Verification

```bash
cargo xtask verify
```

The gate runs formatting, Clippy, the production line budget, all tests, and
95% line coverage.

Run the published benchmarks separately:

```bash
cargo build --release --locked
python benchmarks/stress.py
python benchmarks/paired.py --model <model>
```

Licensed under MIT.
