# Prime Agent integration

The optional package at `integrations/prime-agent` wraps the standalone `wtw`
CLI without reading `.wtw` records or reproducing Rust semantics.

Install it after placing `wtw` on `PATH`:

```bash
prime-agent package install /absolute/path/to/why-this-way/integrations/prime-agent
```

Run `/reload` in an active Prime session. The adapter activates only when the Git
root contains `.wtw/SKILL.md`. A root `csm.toml` always suppresses it;
CSM then owns Prime retrieval and checks while the standalone CLI stays usable.

The adapter exposes `/wtw status`, `/wtw explain`, explicit `/wtw guard`,
and a session-only `/wtw auto explain on|off` toggle. Automatic
`explain` defaults to on and can be disabled at launch with
`--wtw-auto-explain off`. It never exposes repository adoption or semantic
record mutation commands.

All subprocesses use literal argv, the Git root as cwd, cancellation, a timeout,
and a 64 KiB UTF-8 output cap. Nonzero exits, killed processes, and truncation
remain explicit. Injected output is delimited as repository knowledge rather
than higher-priority instructions.
