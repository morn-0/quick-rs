use thiserror::Error;

#[derive(Error, Debug)]
pub enum QuickError {
    #[error("Eval {0}")]
    Eval(String),
    #[error("Call {0}")]
    Call(String),
    #[error("CString {0}")]
    CString(String),
    #[error("Type {0}")]
    Type(i32),
}
