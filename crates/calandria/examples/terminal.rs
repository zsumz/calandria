//! Completion and shared shutdown lifecycle without an async-runtime dependency.

use std::{num::NonZeroUsize, thread};

use calandria::{completion, shutdown_barrier};

fn main() {
    let (completion, completer) = completion();
    let worker = thread::spawn(move || completer.complete("finished"));

    assert_eq!(completion.wait(), Ok("finished"));
    assert!(matches!(worker.join(), Ok(Ok(()))));

    let capacity = NonZeroUsize::new(2)
        .unwrap_or_else(|| panic!("the shutdown capacity is statically nonzero"));
    let (shutdown, mut terminal) = shutdown_barrier(capacity);
    let first = shutdown
        .subscribe(|| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("admit first shutdown observer"));
    let second = shutdown
        .subscribe(|| -> Result<(), ()> { panic!("only the first observer publishes") })
        .unwrap_or_else(|_| panic!("admit second shutdown observer"));

    terminal.complete();

    assert_eq!(first.wait(), Ok(()));
    assert_eq!(second.wait(), Ok(()));
}
