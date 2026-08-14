//! Retained partial frame state owned by the example reactor.

use std::io;

use calandria::{Interest, Readiness};
use mio::net::TcpStream;

pub(super) const MAX_FRAME: usize = 4 * 1_024;

#[derive(Debug)]
pub(super) struct Connection {
    pub(super) stream: TcpStream,
    pub(super) read: Vec<u8>,
    pub(super) write: Vec<u8>,
    pub(super) written: usize,
    pub(super) read_ready: bool,
    pub(super) write_ready: bool,
    pub(super) interest: Interest,
}

impl Connection {
    pub(super) fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            read: Vec::with_capacity(MAX_FRAME + 4),
            write: Vec::with_capacity(MAX_FRAME + 4),
            written: 0,
            read_ready: false,
            write_ready: false,
            interest: Interest::READABLE,
        }
    }

    pub(super) fn observe(&mut self, readiness: Readiness) {
        self.read_ready |=
            readiness.is_readable() || readiness.is_read_closed() || readiness.is_error();
        self.write_ready |=
            readiness.is_writable() || readiness.is_write_closed() || readiness.is_error();
    }

    pub(super) fn has_local_work(&self) -> bool {
        self.read_ready || (self.write_ready && self.written < self.write.len())
    }

    pub(super) fn desired_interest(&self) -> Interest {
        if self.write.is_empty() {
            Interest::READABLE
        } else {
            Interest::READ_WRITE
        }
    }

    pub(super) fn prepare_echo(&mut self) -> io::Result<bool> {
        if self.read.len() < 4 {
            return Ok(false);
        }
        let header: [u8; 4] = self.read[..4]
            .try_into()
            .map_err(|_| io::Error::other("frame header invariant violated"))?;
        let payload = usize::try_from(u32::from_be_bytes(header))
            .map_err(|_| io::Error::other("frame length exceeds platform size"))?;
        if payload > MAX_FRAME {
            return Err(io::Error::other("frame exceeds configured limit"));
        }
        let total = payload.saturating_add(4);
        if self.read.len() < total {
            return Ok(false);
        }
        if self.read.len() != total {
            return Err(io::Error::other("example accepts exactly one frame"));
        }
        self.write.extend_from_slice(&self.read);
        Ok(true)
    }
}
