// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Error types for libsys
//!
//! This module defines error types and status codes used
//! throughout the userspace libraries.

#![no_std]

use core::fmt;

/// Status codes returned by syscalls
///
/// These match the kernel's internal status codes.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Operation completed successfully
    Ok = 0,

    /// Insufficient memory available
    NoMemory = 1,

    /// Operation not supported
    NotSupported = 2,

    /// Invalid arguments provided
    InvalidArgs = 3,

    /// Resource not found
    NotFound = 4,

    /// Operation already in progress
    AlreadyExists = 5,

    /// Operation would block
    WouldBlock = 6,

    /// Access denied
    AccessDenied = 7,

    /// I/O error occurred
    IoError = 8,

    /// System is in bad state
    BadState = 9,

    /// Operation timed out
    TimedOut = 10,

    /// Buffer too small
    BufferTooSmall = 11,

    /// Handle was closed
    HandleClosed = 12,

    /// Resource busy
    Busy = 13,

    /// Internal error
    Internal = 14,

    /// Wrong type for handle
    WrongType = 15,
}

impl Status {
    /// Convert a raw status code to a Status
    pub fn from_raw(raw: i32) -> Self {
        match raw {
            0 => Status::Ok,
            1 => Status::NoMemory,
            2 => Status::NotSupported,
            3 => Status::InvalidArgs,
            4 => Status::NotFound,
            5 => Status::AlreadyExists,
            6 => Status::WouldBlock,
            7 => Status::AccessDenied,
            8 => Status::IoError,
            9 => Status::BadState,
            10 => Status::TimedOut,
            11 => Status::BufferTooSmall,
            12 => Status::HandleClosed,
            13 => Status::Busy,
            14 => Status::Internal,
            15 => Status::WrongType,
            _ => Status::Internal,
        }
    }

    /// Convert to raw status code
    pub fn into_raw(self) -> i32 {
        self as i32
    }

    /// Check if status indicates success
    pub fn is_ok(self) -> bool {
        self == Status::Ok
    }

    /// Check if status indicates an error
    pub fn is_err(self) -> bool {
        self != Status::Ok
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Ok => write!(f, "Operation successful"),
            Status::NoMemory => write!(f, "Insufficient memory"),
            Status::NotSupported => write!(f, "Operation not supported"),
            Status::InvalidArgs => write!(f, "Invalid arguments"),
            Status::NotFound => write!(f, "Resource not found"),
            Status::AlreadyExists => write!(f, "Resource already exists"),
            Status::WouldBlock => write!(f, "Operation would block"),
            Status::AccessDenied => write!(f, "Access denied"),
            Status::IoError => write!(f, "I/O error"),
            Status::BadState => write!(f, "Bad state"),
            Status::TimedOut => write!(f, "Operation timed out"),
            Status::BufferTooSmall => write!(f, "Buffer too small"),
            Status::HandleClosed => write!(f, "Handle closed"),
            Status::Busy => write!(f, "Resource busy"),
            Status::Internal => write!(f, "Internal error"),
            Status::WrongType => write!(f, "Wrong type"),
        }
    }
}

/// Result type for libsys operations
pub type Result<T> = core::result::Result<T, Error>;

/// Error type for libsys operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error {
    /// The status code
    pub status: Status,
}

impl Error {
    /// Create a new error from a status code
    pub fn new(status: Status) -> Self {
        Self { status }
    }

    /// Create an error from a raw status code
    pub fn from_raw(raw: i32) -> Self {
        Self {
            status: Status::from_raw(raw),
        }
    }

    /// Get the status code
    pub fn status(self) -> Status {
        self.status
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.status)
    }
}

impl From<Status> for Error {
    fn from(status: Status) -> Self {
        Self { status }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_conversion() {
        let status = Status::from_raw(0);
        assert_eq!(status, Status::Ok);
        assert!(status.is_ok());
        assert!(!status.is_err());

        let status = Status::from_raw(4);
        assert_eq!(status, Status::NotFound);
        assert!(!status.is_ok());
        assert!(status.is_err());
    }

    #[test]
    fn test_error_creation() {
        let error = Error::new(Status::NotFound);
        assert_eq!(error.status(), Status::NotFound);

        let error = Error::from_raw(7);
        assert_eq!(error.status(), Status::AccessDenied);
    }
}
