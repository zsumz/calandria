//! Portable readiness flags without connection policy.

use core::{
    fmt,
    ops::{BitOr, BitOrAssign},
};

const READABLE: u16 = 1;
const WRITABLE: u16 = 1 << 1;
const READ_CLOSED: u16 = 1 << 2;
const WRITE_CLOSED: u16 = 1 << 3;
const ERROR: u16 = 1 << 4;
const PRIORITY: u16 = 1 << 5;
const AIO: u16 = 1 << 6;
const LIO: u16 = 1 << 7;

/// Read, write, closure, error, and backend readiness observations.
///
/// Readable and writable indicate that a nonblocking operation may make
/// progress. Closure and error flags are backend hints, not terminal policy;
/// the resource owner must attempt its operation and interpret the result.
/// Priority, AIO, and LIO preserve less common observations without teaching
/// the generic poller how a concrete resource should handle them.
#[repr(transparent)]
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct Readiness(u16);

impl Readiness {
    /// No readiness flags.
    pub const EMPTY: Self = Self(0);
    /// Read progress may be possible.
    pub const READABLE: Self = Self(READABLE);
    /// Write progress may be possible.
    pub const WRITABLE: Self = Self(WRITABLE);
    /// The read half may have closed.
    pub const READ_CLOSED: Self = Self(READ_CLOSED);
    /// The write half may have closed.
    pub const WRITE_CLOSED: Self = Self(WRITE_CLOSED);
    /// The resource may have an error pending.
    pub const ERROR: Self = Self(ERROR);
    /// Priority or out-of-band progress may be possible.
    pub const PRIORITY: Self = Self(PRIORITY);
    /// Asynchronous I/O completion may be available.
    pub const AIO: Self = Self(AIO);
    /// List-I/O completion may be available.
    pub const LIO: Self = Self(LIO);

    /// Combines readiness observations.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Removes all observations present in `other`.
    pub const fn remove(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Returns whether every flag in `other` is present.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns whether any flag in `other` is present.
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Returns whether no flags are present.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns whether read progress may be possible.
    pub const fn is_readable(self) -> bool {
        self.0 & READABLE != 0
    }

    /// Returns whether write progress may be possible.
    pub const fn is_writable(self) -> bool {
        self.0 & WRITABLE != 0
    }

    /// Returns whether the read half may have closed.
    pub const fn is_read_closed(self) -> bool {
        self.0 & READ_CLOSED != 0
    }

    /// Returns whether the write half may have closed.
    pub const fn is_write_closed(self) -> bool {
        self.0 & WRITE_CLOSED != 0
    }

    /// Returns whether either half may have closed.
    pub const fn is_closed(self) -> bool {
        self.intersects(Self::READ_CLOSED.union(Self::WRITE_CLOSED))
    }

    /// Returns whether the resource may have an error pending.
    pub const fn is_error(self) -> bool {
        self.0 & ERROR != 0
    }

    /// Returns whether priority progress may be possible.
    pub const fn is_priority(self) -> bool {
        self.0 & PRIORITY != 0
    }

    /// Returns whether asynchronous I/O completion may be available.
    pub const fn is_aio(self) -> bool {
        self.0 & AIO != 0
    }

    /// Returns whether list-I/O completion may be available.
    pub const fn is_lio(self) -> bool {
        self.0 & LIO != 0
    }
}

impl fmt::Debug for Readiness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return formatter.write_str("EMPTY");
        }
        let flags = [
            (Self::READABLE, "READABLE"),
            (Self::WRITABLE, "WRITABLE"),
            (Self::READ_CLOSED, "READ_CLOSED"),
            (Self::WRITE_CLOSED, "WRITE_CLOSED"),
            (Self::ERROR, "ERROR"),
            (Self::PRIORITY, "PRIORITY"),
            (Self::AIO, "AIO"),
            (Self::LIO, "LIO"),
        ];
        let mut separator = "";
        for (flag, name) in flags {
            if self.contains(flag) {
                formatter.write_str(separator)?;
                formatter.write_str(name)?;
                separator = " | ";
            }
        }
        Ok(())
    }
}

impl BitOr for Readiness {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl BitOrAssign for Readiness {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = (*self).union(rhs);
    }
}
