# Architecture

The repository follows ports and adapters, but stays a small workspace rather
than starting with a plugin framework.

```text
Linux procfs/sysfs -> nekohub-agent -> Unix socket -> collector task
                           |                             |
                           v                             v
                  in-memory history              app state/history
                           |                             |
                           v                             v
                  Prometheus endpoint              Ratatui views

OpenSSH config -> inventory and future secure agent bootstrap/administration
```

## Crates

- `nekohub-core`: host identity, raw samples, normalized snapshots, rate
  calculation, inventory discovery, and the collector port. It has no terminal
  or process dependency.
- `nekohub-agent`: native Linux collection, versioned local protocol, bounded
  in-memory history, client adapter, and Prometheus endpoint.
- `nekohub-ssh`: retained OpenSSH adapter for inventory/bootstrap and future
  explicitly administrative operations; it is not used for metric polling.
- `nekohub-tui`: scheduling, demo data, interaction, and presentation.

This separation makes parsers and metric semantics testable without a terminal
or reachable server. It leaves room for remote agent transports and durable
history providers without changing the UI's domain model.

## Concurrency model

The agent has one sampling loop, stores only a bounded history, and broadcasts
new raw samples to streaming clients. Each TUI host owns one long-lived async
task, while results travel through a bounded channel. The UI owns its state and
only applies complete snapshots.

Refresh is a broadcast signal, not another spawned collection. Shutdown is a
watch signal observed by every TUI worker.

## Snapshot semantics

The native agent reads cumulative counters and point-in-time gauges. The agent
and clients calculate CPU and network rates from successive samples. The first
sample therefore shows unknown rates rather than invented zeroes.

Snapshots are immutable and carry collection time and latency. Errors never
erase the last good snapshot; the UI can distinguish offline, stale, and
never-seen hosts as the model evolves.

## Agent and SSH boundaries

The local TUI protocol uses a Unix socket and contains read-only requests only.
Prometheus binds to loopback by default. Remote transport will add an explicit
authenticated pairing protocol before it is enabled.

OpenSSH remains responsible for host-key verification, authentication, jump
hosts, and config resolution when used for inventory or administration. It is
not the continuous monitoring transport.

## Extension rule

Add a capability only when its absence is representable. For example, a host
without systemd is not broken; it simply has no systemd provider. UI screens
depend on normalized capabilities, never directly on a distro name.
