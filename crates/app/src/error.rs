//! One error type for the app: what failed, in words fit for the log.

use std::fmt;

#[derive(Debug)]
pub struct Error(String);

impl Error {
    pub fn new(what: &str) -> Self {
        Self(what.to_owned())
    }

    pub fn caused(what: &str, cause: impl fmt::Debug) -> Self {
        Self(format!("{what}: {cause:?}"))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}
