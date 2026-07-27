# Contributing

All code and documentation are English. Keep the product contract narrow and
fail closed at authority, judge, storage, and graph boundaries.

Run before opening a pull request:

```bash
cargo xtask verify
```

Protocol changes require corresponding architecture, JSON schema, CLI/MCP,
and test updates.
