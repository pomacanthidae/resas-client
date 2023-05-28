use std::fmt;

#[derive(Debug)]
pub enum ErrorKind {
    Fatal,
    Retryable,
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn std::error::Error>>,
    message: Option<String>,
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|e| e.as_ref())
    }
}

impl std::convert::From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Error {
        Error {
            kind: ErrorKind::Fatal,
            source: Some(Box::from(err)),
            message: None,
        }
    }
}

impl std::convert::From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Error {
        Error {
            kind: ErrorKind::Fatal,
            source: Some(Box::from(err)),
            message: None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self.kind {
            ErrorKind::Fatal => write!(f, "Fatal error! ")?,
            ErrorKind::Retryable => write!(f, "Retryable error! ")?,
        }
        if let Some(message) = self.message.as_ref() {
            write!(f, "{}", message)?;
        }
        if let Some(source) = self.source.as_ref() {
            return source.fmt(f);
        }
        Ok(())
    }
}
