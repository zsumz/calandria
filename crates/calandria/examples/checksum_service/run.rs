//! Singular and grouped checksum execution owned by the example application.

use std::error::Error;

use calandria::{
    ReactorGroup, ReactorGroupOutcome, ReactorId, ReactorOutcome, completion, mailbox,
};

use super::service::{Command, Receipt, group_limits, member, reactor, service_limits};

const WORKLOAD: [&[u8]; 6] = [
    b"alpha", b"bravo", b"charlie", b"delta", b"echo", b"foxtrot",
];

pub(super) fn checksum_service() -> Result<(), Box<dyn Error>> {
    let singular = run_singular()?;
    let sharded = run_group()?;
    assert_eq!(normalized(&singular), normalized(&sharded));
    println!(
        "checksum parity preserved for {} requests across singular and grouped execution",
        singular.len()
    );
    Ok(())
}

fn run_singular() -> Result<Vec<Receipt>, Box<dyn Error>> {
    let id = ReactorId::new(0);
    let (parker, notifier) = calandria::thread_parker();
    let (sender, receiver) = mailbox(service_limits(), notifier.wake_handle());
    let handle =
        reactor(id, receiver, parker, notifier.wake_handle()).spawn("checksum-singular")?;
    let mut observations = Vec::with_capacity(WORKLOAD.len());
    for (request, payload) in WORKLOAD.iter().enumerate() {
        let (receipt, complete) = completion();
        sender.try_send(Command::Digest {
            request: fixed(request),
            payload: payload.to_vec(),
            complete,
        })?;
        observations.push(receipt);
    }
    let receipts = observations
        .into_iter()
        .map(calandria::Completion::wait)
        .collect::<Result<Vec<_>, _>>()?;
    sender.try_send_control(Command::Stop)?;
    let exit = handle
        .join()
        .unwrap_or_else(|_| panic!("singular checksum reactor panicked"));
    assert!(matches!(exit.outcome(), ReactorOutcome::Stopped));
    Ok(receipts)
}

fn run_group() -> Result<Vec<Receipt>, Box<dyn Error>> {
    let reactors = 2;
    let group = ReactorGroup::spawn(
        group_limits(reactors),
        "checksum-shard",
        (0..reactors)
            .map(|position| member(ReactorId::new(fixed(position))))
            .collect(),
    )?;
    let ingress = group.handle();
    let mut observations = Vec::with_capacity(WORKLOAD.len());
    for (request, payload) in WORKLOAD.iter().enumerate() {
        let (receipt, complete) = completion();
        let shard = ReactorId::new(fixed(request % reactors));
        ingress.try_send(
            shard,
            Command::Digest {
                request: fixed(request),
                payload: payload.to_vec(),
                complete,
            },
        )?;
        observations.push(receipt);
    }
    let receipts = observations
        .into_iter()
        .map(calandria::Completion::wait)
        .collect::<Result<Vec<_>, _>>()?;
    for position in 0..reactors {
        ingress.try_send_control(ReactorId::new(fixed(position)), Command::Stop)?;
    }
    let exit = group
        .join()
        .unwrap_or_else(|_| panic!("checksum reactor group supervisor panicked"));
    assert_eq!(exit.outcome(), ReactorGroupOutcome::Stopped);
    Ok(receipts)
}

fn normalized(receipts: &[Receipt]) -> Vec<(u64, u64)> {
    receipts
        .iter()
        .map(|receipt| {
            assert!(receipt.shard.get() < 2);
            (receipt.request, receipt.checksum)
        })
        .collect()
}

fn fixed(value: usize) -> u64 {
    u64::try_from(value).unwrap_or_else(|_| panic!("example value must fit in u64"))
}
