# Why This Way Architecture

## Purpose

Why This Way is repository-local governance memory for coding agents. It
preserves explicit choices, rejected alternatives, rationale, and falsifiable
invariants so future changes inherit the reasons and truths that make the
repository correct.

## Public model

WTW owns two durable record kinds:

- A **decision** is an authoritative, durable choice with rationale and at
  least one rejected alternative.
- An **invariant** is an authoritative, durable, falsifiable truth with a
  concrete violation.

Both require semantic IDs, reusable glob scopes, an authority class and
literal quote, two literal evidence fragments, provenance, and status.

## Storage

Consumer repositories commit:

```text
.agent-first/
└── wtw/
    ├── SKILL.md
    └── records/
        ├── decisions/
        │   └── <semantic-id>.toml
        └── invariants/
            └── <semantic-id>.toml
```

`.agent-first/wtw/config.local.toml` selects a local judge and is ignored.
TOML is authoritative. There is no database in version 0.1.

Each semantic relation is one TOML array-of-table entry:

```toml
[[links]]
rel = "upholds"
to = "wtw://invariant/module-write-ownership"
basis = "Direct access keeps write ownership visible"
```

WTW records own only active-voice local edges: `establishes`, `upholds`, and
`supersedes`.

## Collection

The host supplies a task, authoritative source files, and the current Git
diff. Internal `.agent-first/**` files are excluded.

The first isolated judge extracts at most 24 candidates. Deterministic
validation enforces:

1. closed `decision` or `invariant` kind;
2. semantic lowercase ID;
3. authority from a closed class and literal source quote;
4. two literal evidence fragments;
5. valid reusable glob scopes;
6. kind-specific shape;
7. typed, resolved WTW relations with nonempty basis.

A second isolated judge receives the same envelope and proposed candidates.
Only byte-for-byte-equivalent structured candidates from both passes are
stored. Conflicting semantic IDs fail closed.

## Retrieval and guard

`explain` is deterministic and model-free. Exact scope matches rank before
token overlap across title, statement, rationale, violation, and scopes.

`guard` retrieves at most 12 relevant active records for the changed paths,
then asks two isolated judges to confirm only direct contradictions or
violations in the diff. Unknown record URIs, unchanged paths, missing literal
evidence, malformed output, and evaluator failures fail closed.

## Suite graph

`export` emits Agent First Graph v1 nodes and edges. Other foundations own
their active-voice edges:

```text
AVP proof     --proves---------------> WTW invariant
RTW example   --exemplifies----------> WTW decision or invariant
NYA scar      --records_violation_of--> WTW invariant
NWC deferment --tracks_blocker_for----> WTW decision
```

Inverse views such as `proved_by` are derived, never persisted twice.
Standalone health accepts absent external graphs. Suite health requires every
active invariant to have an inbound proof from an active `proof` node.

## Product boundaries

WTW does not preserve implementation examples, historical corrections,
acceptance criteria, or future deferments. RTW, NYA, AVP, and NWC own those
records. WTW understands their graph vocabulary without importing their
runtimes.

## Engineering constitution

```text
Production code:         <= 1,100 LOC
Shared runtime coverage: >= 95%
Packaged entrypoint:     end-to-end smoke tested
```

`cargo xtask verify` is the canonical local, CI, and release gate.
