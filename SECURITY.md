# Security policy

Please report vulnerabilities privately to the maintainers rather than opening
a public issue. The final disclosure address will be added before the first
public release.

Security invariants for contributors:

- OpenSSH host-key verification is never disabled by default.
- Passwords and private keys are never stored by the application.
- Remote probes are read-only, bounded by timeouts, and do not use `sudo`.
- Terminal output from remote hosts is treated as untrusted text.
- Updates are distributed as releases; the running binary does not replace
  itself with code fetched at startup.

