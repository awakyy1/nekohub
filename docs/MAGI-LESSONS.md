# Lessons from MAGI

MAGI is the existing Rust/Ratatui dashboard used as the visual and interaction
reference for this project. It is exceptionally effective for three fixed
nodes. This document records what should influence a general fleet UI without
copying its fictional branding, logo, or three-node geometry.

## What is worth preserving

- **Decision-first screens.** The strongest view does not merely show CPU and
  memory; it explains concentration, the current bottleneck, and where capacity
  remains.
- **Fleet before host.** Operators first need the outlier, then the drill-down.
- **Freshness matters.** A beautifully formatted old value is dangerous. Source,
  age, and collection failure must be visible.
- **Useful history beats duplicated gauges.** A compact availability strip or
  sparkline adds information; another proportional bar often does not.
- **Responsive input is architectural.** Rendering and keyboard handling must
  never wait for remote collection.
- **Measure the data path.** MAGI reduced payloads and changed query semantics
  after observing real latency and scrape behavior. This discipline stays.
- **Color has semantics.** Red is reserved for actual unavailability; high but
  functioning utilization must not look identical to an offline machine.
- **Dense can still be calm.** Aligned labels, fixed-width numbers, restrained
  borders, and a small palette make high information density readable.
- **Panels carry state.** A host card's border and title communicate health
  before the user reads an individual metric.

## What must not be copied

- hard-coded hosts, roles, label names, thresholds, and workload naming rules;
- direct dependence on pre-installed exporters or a central Prometheus;
- a single source file mixing transport, parsing, state, policy, and rendering;
- background work that can overlap when a polling cycle exceeds its interval;
- broad exception swallowing that turns failures into unexplained empty tables;
- self-replacement with code fetched from the network at startup;
- organization-specific operational snapshots inside a public source tree;
- branding or artwork derived from a third-party fictional property.
- one tab per host or a fixed triangular layout, both of which stop scaling
  once the inventory is larger than a handful of machines.

## Product conclusion

The reusable product is not “MAGI, but generic” and not “a remote btop.” It is a
terminal-native decision surface over a Linux fleet. Generic metrics establish
trust; capability providers later turn them into decisions for services,
containers, replication, and other workloads.
