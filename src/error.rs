use std::fmt::{self, Display};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Exception {
    pub name: String,
    pub message: String,
    pub stack: String,
}

impl Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.name.is_empty(), self.message.is_empty()) {
            (false, false) => write!(f, "{}: {}", self.name, self.message)?,
            (true, false) => write!(f, "{}", self.message)?,
            _ => write!(f, "{}", self.name)?,
        }
        if !self.stack.is_empty() {
            write!(f, "\n{}", self.stack)?;
        }
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("execution interrupted")]
    Interrupted,
    #[error("{0}")]
    Exception(Exception),
    #[error("type mismatch: expected {expected}, got tag {got}")]
    Type { expected: &'static str, got: i32 },
    #[error("string contains an interior NUL byte")]
    NulString,
    #[error("{0} returned null")]
    Null(&'static str),
    #[error("{0}")]
    Argument(&'static str),
}
