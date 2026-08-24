//! Bounded reactor and single-owner duty primitives.
//!
//! Calandria separates reusable execution mechanisms from domain policy. The
//! core vocabulary and embedded duty host are available without allocation;
//! owner-local queues and tables use the optional `alloc` feature, enabled by
//! default through `std`.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod exports;

#[cfg(feature = "alloc")]
pub mod batch;
#[cfg(feature = "std")]
pub mod completion;
pub mod host;
#[cfg(feature = "alloc")]
pub mod io;
#[cfg(feature = "std")]
pub mod mailbox;
#[cfg(feature = "std")]
pub mod reactor;
#[cfg(feature = "alloc")]
pub mod resource;
pub mod retained;
#[cfg(feature = "std")]
pub mod shutdown;
#[cfg(feature = "std")]
mod sync;
#[cfg(all(feature = "std", test, calandria_loom))]
mod sync_test;
pub mod time;
#[cfg(feature = "alloc")]
pub mod timer;
pub mod turn;
#[cfg(feature = "std")]
pub mod wake;

pub use exports::*;
