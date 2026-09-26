# nekoHub

Linux fleet management, designed for the terminal.

`nekoHub` is a terminal application for people who operate several Linux
machines. Its small native agent reads Linux metrics locally and turns them
into a fast fleet overview, useful drill-downs, and Prometheus data for Grafana.

## What exists today

This repository is the architectural seed and first executable vertical slice:

- discovers concrete host aliases from `~/.ssh/config`, including local
  `Include` directives;
- includes a read-only Rust agent that parses `/proc` and `/sys` without shell
  commands;
- keeps a short in-memory history and serves the TUI through a local Unix
  socket;
- exports Prometheus metrics on `127.0.0.1:9876` for Grafana integration;
- keeps OpenSSH inventory for future pairing and administration, but does not
  use SSH for continuous metric collection;
- renders a responsive Ratatui fleet overview and host detail;
- includes a deterministic demo mode and parser/state tests.
- starts with an animated choice between monitoring the current machine and an
  OpenSSH host;
- collects the current Linux machine through `nekohub-agent` without SSH;
- presents concrete aliases from `~/.ssh/config` in the remote picker.

## Run

Requires Linux and a current stable Rust toolchain when running from source.

```bash
cargo run -p nekohub-agent --bin nekohub-agent -- --socket /tmp/nekohub.sock
cargo run -p nekohub-tui --bin nekohub -- --agent-socket /tmp/nekohub.sock
cargo run -p nekohub-tui --bin nekohub -- --demo
cargo run -p nekohub-tui --bin nekohub -- --once --local --agent-socket /tmp/nekohub.sock
```

On first launch, choose **Monitor this machine** and confirm the one-time agent
setup. nekoHub displays an animated progress screen, validates real Linux
metrics, and opens the local dashboard. Once setup succeeds, later launches go
directly to that dashboard. If the service is unavailable, setup pauses with
recovery commands and does not mark onboarding as complete.

The remote picker already discovers aliases in `~/.ssh/config`; secure remote
agent pairing is the next milestone, and SSH metric collection is disabled.

Keys: `j`/`k` or arrows move, `Enter` selects, `Esc` goes back, `r` refreshes,
and `q` quits.

## Debian package

Configure the official nekoHub repository once:

```bash
curl -fsSL https://awakyy1.github.io/nekohub/install.sh | sudo sh
```

Then install and update nekoHub through APT:

```bash
sudo apt install nekohub
sudo apt upgrade
```

The `nekohub` package installs the `nekohub-agent` service automatically. Check
it with `systemctl status nekohub-agent`.

Alternatively, install downloaded builds directly:

```bash
sudo apt install ./nekohub-agent_0.2.1_amd64.deb ./nekohub_0.2.1_amd64.deb
nekohub
```

The leading `./` matters when installing local packages.

## Prometheus and Grafana

The agent exposes Prometheus text metrics locally at
`http://127.0.0.1:9876/metrics`. Add it to Prometheus:

```yaml
scrape_configs:
  - job_name: nekohub
    static_configs:
      - targets: ["127.0.0.1:9876"]
```

Grafana then uses Prometheus as its data source. Binding the endpoint to a
non-loopback address is configurable, but V1 has no TLS or authentication; do
not expose it directly to the public internet.

## Product boundary

The MVP is deliberately not a Prometheus replacement, a configuration manager,
or a remote shell. It should answer three questions reliably:

1. Which host needs attention?
2. Why does it need attention?
3. What changed recently?

See [MVP](docs/MVP.md), [architecture](docs/ARCHITECTURE.md), and the
[visual/product lessons taken from the Rust MAGI reference](docs/MAGI-LESSONS.md).

## Safety

The agent is read-only and runs as a dedicated unprivileged system user. It
does not execute shell commands, mutate the host, or auto-update executable
code. Host-key verification and SSH authentication remain delegated to
OpenSSH for the operations that use it.

## License

MIT.
