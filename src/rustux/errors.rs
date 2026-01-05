// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Common error types used throughout the kernel

#![no_std]

use crate::rustux::types::*;

/// Result type for operations that can fail
pub type Result<T = ()> = core::result::Result<T, Error>;

/// Common error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Ok = 0,
    Err = -1,
    InvalidArgs = -2,
    BadHandle = -3,
    BadState = -4,
    NotSupported = -5,
    NoMemory = -6,
    TimedOut = -7,
    NotFound = -8,
    AlreadyExists = -9,
    AccessDenied = -10,
    Io = -11,
    Internal = -12,
}

impl Error {
    /// Convert error to status code
    pub fn to_status(self) -> Status {
        self as Status
    }

    /// Convert status code to error
    pub fn from_status(status: Status) -> Self {
        match status {
            0 => Error::Ok,
            -1 => Error::Err,
            -2 => Error::InvalidArgs,
            -3 => Error::BadHandle,
            -4 => Error::BadState,
            -5 => Error::NotSupported,
            -6 => Error::NoMemory,
            -7 => Error::TimedOut,
            -8 => Error::NotFound,
            -9 => Error::AlreadyExists,
            -10 => Error::AccessDenied,
            -11 => Error::Io,
            -12 => Error::Internal,
            _ => Error::Internal,
        }
    }
}

impl From<Status> for Error {
    fn from(status: Status) -> Self {
        Self::from_status(status)
    }
}

impl From<Error> for Status {
    fn from(err: Error) -> Self {
        err.to_status()
    }
}
