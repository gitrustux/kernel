// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Rustux IPC Library (libipc)
//!
//! This library provides high-level wrappers for IPC primitives:
//! - Channels for message passing
//! - Events and EventPairs for signaling
//! - Ports for packet delivery
//!
//! # Examples
//!
//! ```no_run
//! use libipc::*;
//! use libsys::*;
//!
//! fn main() -> Result<()> {
//!     // Create a channel
//!     let (tx, rx) = Channel::create()?;
//!
//!     // Send a message
//!     tx.write(b"Hello, World!", &[])?;
//!
//!     // Receive a message
//!     let mut buf = [0u8; 64];
//!     let mut handles = vec![];
//!     let n = rx.read(&mut buf, &mut handles)?;
//!
//!     println!("Received: {}", core::str::from_utf8(&buf[..n]).unwrap());
//!
//!     Ok(())
//! }
//! ```

#![no_std]

pub mod channel;
pub mod event;
pub mod port;

// Re-export commonly used types
pub use channel::{Channel, ChannelReadArgs, ChannelWriteArgs, ChannelCallEtcArgs};
pub use event::{Event, EventPair};
pub use port::{Port, Packet, PacketWaitResult};
