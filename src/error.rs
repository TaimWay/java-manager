use std::fmt;
use std::io;

#[derive(Debug)]
pub enum JavaError {
    InvalidJavaPath(String),
    NotFound(String),
    IoError(io::Error),
    ExecuteError(String),
    RuntimeError(String),
    Other(String)
}

impl fmt::Display for JavaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            JavaError::InvalidJavaPath(msg) => write!(f, "Invalid Java path: {}", msg),
            JavaError::NotFound(msg) => write!(f, "Not found: {}", msg),
            JavaError::IoError(err) => write!(f, "IO error: {}", err),
            JavaError::ExecuteError(msg) => write!(f, "Execute error: {}", msg),
            JavaError::RuntimeError(msg) => write!(f, "Runtime error: {}", msg),
            JavaError::Other(msg) => write!(f, "Other error: {}", msg),
        }
    }
}

impl std::error::Error for JavaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            JavaError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for JavaError {
    fn from(err: io::Error) -> Self {
        JavaError::IoError(err)
    }
}