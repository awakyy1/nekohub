# Roadmap

## 0.1 — visual and packaging foundation

- harden OpenSSH inventory and `Include` behavior;
- golden UI tests at compact and wide terminal sizes;
- release binaries and installation documentation.

## 0.2 — native agent foundation

- collect host fundamentals directly from procfs and sysfs;
- local versioned protocol and bounded in-memory history;
- systemd service and separate Debian package;
- Prometheus endpoint as a first-class Grafana integration;
- switch local TUI monitoring from shell probes to the agent.

## 0.3 — secure fleet pairing

- authenticated remote agent pairing and encrypted transport;
- offline/stale state machine and per-host retry backoff;
- configuration file for groups, tags, thresholds, and refresh intervals;
- validate metric semantics across major distro families and kernels.

## 0.4 — host diagnosis

- process list and drill-down;
- mounts and block devices rather than root filesystem only;
- interface-aware network metrics;
- pressure stall information when supported;
- capability and permission diagnostics.

## 0.5 — services and logs

- systemd provider, failed-unit summary, and bounded journal view;
- OpenRC provider;
- safe log streaming with pause, filter, and dropped-line counters.

## 0.6 — workloads

- Docker and Podman providers;
- normalized container/workload model;
- fleet-wide workload placement and health views.

## Later

- optional durable history adapters and community Grafana dashboards;
- explicit, audited actions with preview and confirmation;
- extension API only after two real providers expose the right boundary;
- saved views, alert handoff, and remote collaboration workflows.
