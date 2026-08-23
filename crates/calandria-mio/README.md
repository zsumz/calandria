# calandria-mio

Mio readiness mechanics for concrete Calandria reactors.

`calandria-mio` owns selector polling, source registration, waking, and bounded
translation between backend tokens and Calandria resource identities.

```toml
[dependencies]
calandria = "=0.0.1-rc.1"
calandria-mio = "=0.0.1-rc.1"
```

The concrete reactor continues to own its sources, resource table, readiness
destination, retained I/O progress, protocol state, fairness, and shutdown
policy. Readiness is a hint; actual I/O progress and `WouldBlock` determine
state transitions.

Run the bounded framed-reactor example from a checkout:

```sh
cargo run -p calandria-mio --example framed_reactor --locked
```

[Repository](https://github.com/zsumz/calandria) ·
[API documentation](https://docs.rs/calandria-mio)
