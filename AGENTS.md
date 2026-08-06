# Why This Way Engineering Guide

All repository artifacts must be written in English.

## Product contract

Why This Way exposes two public concepts, the decision and the invariant, and
seven operations:

1. `wtw collect`
2. `wtw explain`
3. `wtw guard`
4. `wtw show`
5. `wtw health`
6. `wtw supersede`
7. `wtw export`

`wtw init` installs consumer assets. `wtw mcp` exposes the same core.

There is no manual add command. A durable record may be created only from an
authoritative source through two isolated, identical, evidence-bounded judge
passes.

## Engineering constitution

1. Production code under `src/` must remain at or below 1,100 code lines as
   measured by `tokei`.
2. Shared runtime line coverage must remain at or above 95 percent without
   rounding. The process entrypoint is verified end to end.
3. Test code is unlimited and must live under `tests/`.
4. Git TOML files are the durable source of truth.
5. Graph JSON is a derived federation protocol, never unique knowledge.
6. CLI and MCP call the same Rust operations.
7. Judge, authority, relation, storage, and protocol failures fail closed.
8. `.wtw/**` never enters WTW's own collection or guard envelope.

The larger line budget than the single-concept sibling tools is intentional:
WTW owns two independently validated record shapes plus the suite graph
protocol. Do not expand it without deleting equivalent complexity.

## Change discipline

Prefer the smallest complete implementation. New record kinds, relation verbs,
judge authority classes, and graph schema changes are public protocol changes.

Before reporting implementation complete, run `cargo xtask verify`.


## Optional Prime Agent adapter

`integrations/prime-agent` is a thin optional host adapter. It may invoke only
the `wtw` CLI with literal argv and must never parse semantic records or
reimplement Rust behavior. It activates only for `.wtw/SKILL.md` and
must remain completely inactive when the Git root contains `csm.toml`. CSM has
absolute Prime-integration precedence.

When changing the adapter, run `npm ci`, `npm test`, `npm run typecheck`, and
`npm pack --dry-run` from `integrations/prime-agent` in addition to
`cargo xtask verify`.
