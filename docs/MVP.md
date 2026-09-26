# MVP scope

## Audience and job

The first user is a sysadmin, homelabber, or SRE with roughly 3-50 Linux hosts
already reachable through OpenSSH. They want to find the machine that deserves
attention without opening a browser or maintaining an observability stack.

## In scope

- inventory from concrete aliases in OpenSSH config;
- bounded concurrent polling with timeout, freshness, and explicit errors;
- fleet overview: reachability, CPU, memory, root filesystem, load, and age;
- host detail: identity, uptime, network throughput, history, and the raw reason
  for an unhealthy state;
- progressive degradation when a metric is unavailable;
- Debian/Ubuntu and RHEL-family validation first, then other Linux families;
- demo mode, one-shot text output, tests, CI, and documented data semantics.

## Explicitly out of scope for 0.1

- installing a remote agent;
- mutating hosts or running arbitrary user commands;
- alerts and notification delivery;
- long-term metrics storage;
- Prometheus as a required dependency;
- Docker/Podman, systemd/OpenRC, process tables, and logs;
- Windows or BSD targets.

These are exclusions in sequence, not claims that the product will never have
them. Host fundamentals must be trustworthy before workload-specific surfaces
are added.

## Success criteria

- first useful frame in under two seconds for reachable hosts on a local network;
- keyboard response is independent of SSH latency;
- at most one collection is in flight per host;
- an offline or slow host cannot stall the other hosts;
- every displayed number has a documented source and unit;
- no remote write, package installation, or privilege escalation;
- a 3-host and a 30-host inventory remain readable at 100x30 and 160x45.

## Next vertical slices

1. Processes: top CPU/memory, search, and safe signal actions only after an
   explicit mutation policy exists.
2. Services: systemd capability detection, failed units, and journal excerpts;
   OpenRC through a separate provider.
3. Containers: Docker and Podman providers behind a shared workload model.
4. Logs: bounded, filtered streaming with redaction and backpressure.
5. Optional history providers, including Prometheus, without making them a
   prerequisite for the fleet view.

