# calandria

Bounded reactor primitives with explicit ownership and deterministic progress.

`calandria` provides the runtime-neutral core: bounded turns, hosting,
mailboxes, timers, resource identities, lifecycle, and dedicated reactor
ownership.

```toml
[dependencies]
calandria = "=0.0.1-rc.2"
```

```rust
use core::convert::Infallible;
use calandria::{Duty, Moment, Turn, WorkCount};

#[derive(Debug)]
struct Worker {
    remaining: u64,
}

impl Duty for Worker {
    type Error = Infallible;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        if self.remaining == 0 {
            return Ok(Turn::stopped(WorkCount::ZERO));
        }

        self.remaining -= 1;
        Ok(Turn::runnable(WorkCount::new(1)))
    }
}
```

Each machine and resource has one mutable owner. Retained work is bounded, and
terminal states remain explicit. Calandria does not provide an async runtime,
a general executor, or protocol policy.

[Repository](https://github.com/zsumz/calandria) ·
[API documentation](https://docs.rs/calandria)
