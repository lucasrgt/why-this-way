# Why This Way for Prime Agent

This optional capability package is a thin adapter around the standalone `wtw`
Rust CLI. It adds bounded automatic `explain`, explicit operator commands, and a
conditional model skill without reading semantic records or reimplementing
Why This Way behavior.

## Install

Install `wtw` on `PATH`, then run:

```bash
prime-agent package install /absolute/path/to/why-this-way/integrations/prime-agent
```

Use `/reload` in a live Prime session. Set `WTW_BIN` or pass
`--wtw-bin /absolute/path/to/wtw` when needed.

## Activation and precedence

The package activates only when the Git root contains `.wtw/SKILL.md`. It is
fully suppressed when `<git-root>/csm.toml` exists, even if the standalone marker
also remains. CSM then owns Prime retrieval and verification; direct standalone
CLI use remains available. In inactive repositories the package invokes no
`wtw` process, exposes no command or skill, and paints no status.

## Surface

- ``/wtw explain <task>` and `/wtw guard [--base=REF] <task>``
- `/wtw status`
- `/wtw auto explain on|off`

Automatic `explain` is enabled by default and can be disabled at launch with
`--wtw-auto-explain off`. Checks are always explicit. The adapter exposes no
repository adoption or semantic-record mutation command.

Every process uses a literal argv array, the resolved Git root as cwd, a
configurable timeout, cancellation, control-sequence sanitization, and a 64 KiB
UTF-8 output cap. Nonzero exits, cancellation, and truncation remain visible.
Repository output is delimited as lower-priority project knowledge.
