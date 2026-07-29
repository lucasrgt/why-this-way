# Security

Why This Way executes the local judge command selected by the repository user.
Treat `.wtw/config.local.toml` as executable configuration and
never commit credentials there.

WTW sends the configured judge only the bounded collection or guard envelope.
The judge runs in a fresh temporary working directory. No hosted service or
telemetry is part of the product.

Report vulnerabilities privately through GitHub Security Advisories for this
repository.
