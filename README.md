# nekoHub

Linux fleet management, designed for the terminal.

`nekoHub` is an agentless terminal application for people who operate several
Linux machines. It starts from the SSH setup they already trust and turns raw
host data into a fast fleet overview and useful drill-downs.

## What exists today

This repository is the architectural seed and first executable vertical slice:

- discovers concrete host aliases from `~/.ssh/config`, including local
  `Include` directives;
- uses the system OpenSSH client, so aliases, keys, agent, `ProxyJump`,
  `known_hosts`, and other established SSH policy remain authoritative;
- runs one read-only shell probe over SSH and parses `/proc`, `/sys`, and `df`;
- collects hosts concurrently, but never overlaps two collections for the same
  host;
- renders a responsive Ratatui fleet overview and host detail;
- includes a deterministic demo mode and parser/state tests.
- starts with an animated choice between monitoring the current machine and an
  OpenSSH host;
- collects the current Linux machine directly without requiring SSH;
- presents concrete aliases from `~/.ssh/config` in the remote picker.

## Run

Requires a current stable Rust toolchain and an `ssh` executable.

```bash
cargo run -p nekohub-tui --bin nekohub
cargo run -p nekohub-tui --bin nekohub -- --demo
cargo run -p nekohub-tui --bin nekohub -- --host my-vps --host home-server
cargo run -p nekohub-tui --bin nekohub -- --once --local
cargo run -p nekohub-tui --bin nekohub -- --once --host my-vps
```

On launch, choose **Monitor this machine** for direct, read-only Linux metrics,
or **Connect to a remote machine** to select an alias discovered in
`~/.ssh/config`. Use `--ssh-config PATH` to select a different file and repeat
`--host ALIAS` to provide an explicit remote list.

Keys: `j`/`k` or arrows move, `Enter` selects, `Esc` goes back, `r` refreshes,
and `q` quits.

## Debian package

Install a downloaded build with:

```bash
sudo apt install ./nekohub_0.1.0_amd64.deb
nekohub
```

The leading `./` matters: this installs a local package. Supporting
`sudo apt install nekohub` without a file requires publishing and signing an
APT repository. The package is intentionally named `nekohub`, so the final
repository command will remain `sudo apt install nekohub`.

## Product boundary

The MVP is deliberately not a Prometheus replacement, a configuration manager,
or a remote shell. It should answer three questions reliably:

1. Which host needs attention?
2. Why does it need attention?
3. What changed recently?

See [MVP](docs/MVP.md), [architecture](docs/ARCHITECTURE.md), and the
[visual/product lessons taken from the Rust MAGI reference](docs/MAGI-LESSONS.md).

## Safety

The collector is read-only. It does not install an agent, use `sudo`, mutate a
host, or auto-update executable code. Host-key verification and authentication
are delegated to OpenSSH.

## License

MIT.
