//! Nonempty readiness interests armed for one resource.

use core::{
    fmt,
    num::NonZeroU16,
    ops::{BitOr, BitOrAssign},
};

const READABLE: u16 = 1;
const WRITABLE: u16 = 1 << 1;
const PRIORITY: u16 = 1 << 2;
const AIO: u16 = 1 << 3;
const LIO: u16 = 1 << 4;

/// Nonempty set of useful progress requested from a readiness backend.
///
/// The representation is extensible: backends may reject interests that their
/// current platform cannot express, while owners can retain one portable type.
#[repr(transparent)]
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Interest(NonZeroU16);

impl Interest {
    /// Read progress only.
    pub const READABLE: Self = Self::from_bits(READABLE);
    /// Write progress only.
    pub const WRITABLE: Self = Self::from_bits(WRITABLE);
    /// Either read or write progress.
    pub const READ_WRITE: Self = Self::from_bits(READABLE | WRITABLE);
    /// Priority or out-of-band progress.
    pub const PRIORITY: Self = Self::from_bits(PRIORITY);
    /// Asynchronous I/O completion.
    pub const AIO: Self = Self::from_bits(AIO);
    /// List-I/O completion.
    pub const LIO: Self = Self::from_bits(LIO);

    /// Combines two nonempty interest sets.
    pub const fn union(self, other: Self) -> Self {
        Self::from_bits(self.0.get() | other.0.get())
    }

    /// Removes interests, returning `None` when the result would be empty.
    pub const fn remove(self, other: Self) -> Option<Self> {
        match NonZeroU16::new(self.0.get() & !other.0.get()) {
            Some(bits) => Some(Self(bits)),
            None => None,
        }
    }

    /// Returns whether every interest in `other` is present.
    pub const fn contains(self, other: Self) -> bool {
        self.0.get() & other.0.get() == other.0.get()
    }

    /// Returns whether any interest in `other` is present.
    pub const fn intersects(self, other: Self) -> bool {
        self.0.get() & other.0.get() != 0
    }

    /// Returns whether read readiness is requested.
    pub const fn is_readable(self) -> bool {
        self.0.get() & READABLE != 0
    }

    /// Returns whether write readiness is requested.
    pub const fn is_writable(self) -> bool {
        self.0.get() & WRITABLE != 0
    }

    /// Returns whether priority readiness is requested.
    pub const fn is_priority(self) -> bool {
        self.0.get() & PRIORITY != 0
    }

    /// Returns whether asynchronous I/O completion is requested.
    pub const fn is_aio(self) -> bool {
        self.0.get() & AIO != 0
    }

    /// Returns whether list-I/O completion is requested.
    pub const fn is_lio(self) -> bool {
        self.0.get() & LIO != 0
    }

    const fn from_bits(bits: u16) -> Self {
        match NonZeroU16::new(bits) {
            Some(bits) => Self(bits),
            None => panic!("readiness interest must be nonempty"),
        }
    }
}

impl BitOr for Interest {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl BitOrAssign for Interest {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = (*self).union(rhs);
    }
}

impl fmt::Debug for Interest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let flags = [
            (Self::READABLE, "READABLE"),
            (Self::WRITABLE, "WRITABLE"),
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
