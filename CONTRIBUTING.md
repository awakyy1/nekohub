# Contributing

Start with an issue that describes the operator problem and the evidence that it
exists. New metrics must document their source, unit, failure mode, and meaning
across kernels or distros.

Before opening a change:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Keep remote collection read-only. Do not add `sudo`, password handling, silent
host-key acceptance, or startup code execution from the network.

