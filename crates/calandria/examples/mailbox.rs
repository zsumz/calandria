//! Publishes control and work commands through a bounded reactor mailbox.

use std::{
    io,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use calandria::{LaneLimits, MailboxLimits, Retained, RetainedBytes, WakeHandle, mailbox};

#[derive(Debug)]
enum Command {
    Shutdown,
    Work(Vec<u8>),
}

impl Retained for Command {
    fn retained_bytes(&self) -> RetainedBytes {
        match self {
            Self::Shutdown => RetainedBytes::ZERO,
            Self::Work(payload) => RetainedBytes::try_from(payload.capacity())
                .unwrap_or_else(|_| panic!("example payload capacity must fit in u64")),
        }
    }
}

fn main() -> io::Result<()> {
    let wake_count = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&wake_count);
    let wake = WakeHandle::new(move || {
        observed.fetch_add(1, Ordering::Relaxed);
        Ok(())
    });
    let limits = MailboxLimits::new(
        LaneLimits::new(nonzero_usize(8), RetainedBytes::new(1_024)),
        LaneLimits::new(nonzero_usize(1_024), RetainedBytes::new(16 * 1_024 * 1_024)),
    );
    let (sender, mut receiver) = mailbox(limits, wake);

    sender
        .try_send(Command::Work(vec![1, 2, 3]))
        .map_err(|error| io::Error::other(error.to_string()))?;
    sender
        .try_send_control(Command::Shutdown)
        .map_err(|error| io::Error::other(error.to_string()))?;

    let mut commands = Vec::new();
    let report = receiver.drain_into(&mut commands, nonzero_usize(16));
    println!("{report:?}: {commands:?}");
    println!("backend wakes: {}", wake_count.load(Ordering::Relaxed));
    Ok(())
}

fn nonzero_usize(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("example limit must be nonzero"))
}
