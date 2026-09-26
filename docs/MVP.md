# MVP scope

## Audience and job

The first user is a sysadmin, homelabber, or SRE with roughly 3-50 Linux hosts.
They want to find the machine that deserves attention without opening a browser
or being forced to maintain a large observability stack.

## In scope

- inventory from concrete aliases in OpenSSH config;
- a small read-only native Linux agent with a versioned local protocol;
- bounded history and Prometheus-compatible metrics for Grafana;
- bounded concurrent polling with timeout, freshness, and explicit errors;
- fleet overview: reachability, CPU, memory, root filesystem, load, and age;
- host detail: identity, uptime, network throughput, history, and the raw reason
  for an unhealthy state;
- progressive degradation when a metric is unavailable;
- Debian/Ubuntu and RHEL-family validation first, then other Linux families;
- demo mode, one-shot text output, tests, CI, and documented data semantics.

## Explicitly out of scope for the agent V1

- secure remote pairing and transport;
- mutating hosts or running arbitrary user commands;
- alerts and notification delivery;
- long-term metrics storage;
- requiring Prometheus or Grafana for the terminal experience;
- Docker/Podman, systemd/OpenRC data, process tables, and logs;
- Windows or BSD targets.

These are exclusions in sequence, not claims that the product will never have
them. Host fundamentals must be trustworthy before workload-specific surfaces
are added.

## Success criteria

- first useful frame in under two seconds for the local machine;
- keyboard response is independent of network latency;
- at most one collection is in flight per host;
- an offline or slow host cannot stall the other hosts;
- every displayed number has a documented source and unit;
- no remote mutation or privilege escalation during monitoring;
- a 3-host and a 30-host inventory remain readable at 100x30 and 160x45.

## Next vertical slices

1. Secure remote agent pairing and encrypted transport.
2. Processes: top CPU/memory, search, and safe signal actions only after an
   explicit mutation policy exists.
3. Services: systemd capability detection, failed units, and journal excerpts;
   OpenRC through a separate provider.
4. Containers: Docker and Podman providers behind a shared workload model.
5. Logs: bounded, filtered streaming with redaction and backpressure.
6. Durable history providers without making them a prerequisite for the fleet
   view; Prometheus export remains a first-class integration.
