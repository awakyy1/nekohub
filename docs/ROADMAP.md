# Roadmap

## 0.1 — trustworthy host loop

- harden OpenSSH inventory and `Include` behavior;
- validate probe semantics across major distro families and kernels;
- offline/stale state machine and per-host retry backoff;
- configuration file for groups, tags, thresholds, and refresh intervals;
- golden UI tests at compact and wide terminal sizes;
- release binaries and installation documentation.

## 0.2 — host diagnosis

- process list and drill-down;
- mounts and block devices rather than root filesystem only;
- interface-aware network metrics;
- pressure stall information when supported;
- capability and permission diagnostics.

## 0.3 — services and logs

- systemd provider, failed-unit summary, and bounded journal view;
- OpenRC provider;
- safe log streaming with pause, filter, and dropped-line counters.

## 0.4 — workloads

- Docker and Podman providers;
- normalized container/workload model;
- fleet-wide workload placement and health views.

## Later

- optional Prometheus/history adapter;
- explicit, audited actions with preview and confirmation;
- extension API only after two real providers expose the right boundary;
- saved views, alert handoff, and remote collaboration workflows.

