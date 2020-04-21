use std::fmt;
use std::fmt::Display;

#[derive(Debug)]
pub enum DictError {
    IoError(::std::io::Error),
    EncodingError(::std::string::FromUtf8Error),
    InvalidBase64,
    SyntaxError(&'static str),
    NoMatch(&'static str),
}

impl Display for DictError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DictError")
    }
}

impl std::error::Error for DictError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            DictError::IoError(ref e) => Some(e),
            DictError::EncodingError(ref e) => Some(e),
            DictError::InvalidBase64 => None,
            DictError::SyntaxError(ref _e) => None,
            DictError::NoMatch(ref _e) => None,
        }
    }
}

impl From<::std::io::Error> for DictError {
    fn from(err: ::std::io::Error) -> DictError {
        DictError::IoError(err)
    }
}

impl From<::std::string::FromUtf8Error> for DictError {
    fn from(err: ::std::string::FromUtf8Error) -> DictError {
        DictError::EncodingError(err)
    }
}
