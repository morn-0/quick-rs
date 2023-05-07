use thiserror::Error;

#[derive(Error, Debug)]
pub enum EvalError {
    #[error("CodeError {0}")]
    CodeError(String),
    #[error("ExecuteError {0}")]
    ExecuteError(String),
    #[error("CStringError {0}")]
    CStringError(String),
}
