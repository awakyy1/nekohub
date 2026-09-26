# Architecture

The repository follows ports and adapters, but stays a small workspace rather
than starting with a plugin framework.

```text
OpenSSH config -> inventory -> one collector task per host -> latest snapshot
                                      |                           |
                                      v                           v
                                ssh + shell probe          app state/history
                                                                  |
                                                                  v
                                                            Ratatui views
```

## Crates

- `fleet-core`: host identity, raw samples, normalized snapshots, rate
  calculation, inventory discovery, and the collector port. It has no terminal
  or process dependency.
- `fleet-ssh`: OpenSSH process adapter and the versioned, read-only Linux probe.
- `fleet-tui`: scheduling, demo data, interaction, and presentation.

This separation makes parsers and metric semantics testable without a terminal
or reachable server. It also leaves room for a native SSH adapter or a
Prometheus history adapter without changing the UI's domain model.

## Concurrency model

Each host owns one long-lived async task. The task awaits a collection before it
waits for the next interval, so a slow request cannot create an unbounded queue
or overlap itself. Hosts still run concurrently. Results travel through a
bounded channel; the UI owns its state and only applies complete snapshots.

Refresh is a broadcast signal, not another spawned collection. Shutdown is a
watch signal observed by every host task.

## Snapshot semantics

The remote probe returns cumulative counters and point-in-time gauges. The
controller calculates CPU and network rates from two successful samples. This
keeps the remote command short and avoids sleeping on every host. The first
sample therefore shows unknown rates rather than invented zeroes.

Snapshots are immutable and carry collection time and latency. Errors never
erase the last good snapshot; the UI can distinguish offline, stale, and
never-seen hosts as the model evolves.

## SSH boundary

The MVP invokes the user's OpenSSH executable with the selected alias. OpenSSH
remains responsible for authentication, host-key verification, jump hosts,
socket multiplexing, and config resolution. The application never accepts or
stores passwords.

The probe protocol begins with a version field and uses one `key=value` record
per line. Unknown keys are ignored, which permits additive evolution. A future
protocol change that changes meaning must increment the version.

## Extension rule

Add a capability only when its absence is representable. For example, a host
without systemd is not broken; it simply has no systemd provider. UI screens
depend on normalized capabilities, never directly on a distro name.

