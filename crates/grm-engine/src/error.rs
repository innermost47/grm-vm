// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! What the machine says when it cannot do what it was told.

use std::fmt;

/// An error said to the human: what went wrong, in words.
#[derive(Debug, Clone, PartialEq)]
pub struct GrmError {
    pub message: String,
    /// Already says the file and line it comes from (a file used by another).
    pub located: bool,
}

impl GrmError {
    pub fn new(message: impl Into<String>) -> Self {
        GrmError { message: message.into(), located: false }
    }
}

impl fmt::Display for GrmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for GrmError {}

pub type Result<T> = std::result::Result<T, GrmError>;

/// Shorthand: `bail!("...")` returns an error said in words.
#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => { return Err($crate::error::GrmError::new(format!($($arg)*))) };
}
