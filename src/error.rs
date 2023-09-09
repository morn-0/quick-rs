use thiserror::Error;

#[derive(Error, Debug)]
pub enum QuickError {
    #[error("CodeError {0}")]
    CodeError(String),
    #[error("EvalError {0}")]
    EvalError(String),
    #[error("CallError {0}")]
    CallError(String),
    #[error("CStringError {0}")]
    CStringError(String),
    #[error("UnsupportedTypeError {0}")]
    UnsupportedTypeError(i32),
}
