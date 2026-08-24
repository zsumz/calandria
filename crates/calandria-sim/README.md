# calandria-sim

Deterministic simulation for Calandria duties and reactors.

`calandria-sim` provides static topology, virtual time, bounded scheduling,
scripted outcomes, observations, causal traces, and replay without threads,
wall time, real I/O, or ambient entropy.

```toml
[dependencies]
calandria = "=0.0.1-rc.2"
calandria-sim = "=0.0.1-rc.2"
```

Production and simulation share the same `Duty`, `Turn`, time, and lifecycle
contracts. The simulation owns model and scheduling state while preserving
explicit count, byte, action, and time limits.

Run the deterministic duty example from a checkout:

```sh
cargo run -p calandria-sim --example duty_simulation --locked
```

[Repository](https://github.com/zsumz/calandria) ·
[API documentation](https://docs.rs/calandria-sim)
