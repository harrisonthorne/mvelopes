use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

// TODO auto-fixable errors?

pub type SilverfoxResult<T> = Result<T, SilverfoxError>;

/// SilverfoxError is an enum for all possible custom errors that silverfox can throw. It is a
/// wrapper of sorts.
#[derive(Debug)]
pub enum SilverfoxError {
    Basic(String),
    Parse(ParseError),
    Validation(ValidationError),
    Processing(ProcessingError),
    File(PathBuf, std::io::Error),
    Csv(csv::Error),
}

impl Error for SilverfoxError {}

impl From<ParseError> for SilverfoxError {
    fn from(err: ParseError) -> Self {
        Self::Parse(err)
    }
}

impl From<ValidationError> for SilverfoxError {
    fn from(err: ValidationError) -> Self {
        Self::Validation(err)
    }
}

impl From<ProcessingError> for SilverfoxError {
    fn from(err: ProcessingError) -> Self {
        Self::Processing(err)
    }
}

impl From<csv::Error> for SilverfoxError {
    fn from(err: csv::Error) -> Self {
        Self::Csv(err)
    }
}

impl fmt::Display for SilverfoxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SilverfoxError::Basic(s) => s.fmt(f),
            SilverfoxError::Validation(v) => v.fmt(f),
            SilverfoxError::Parse(p) => p.fmt(f),
            SilverfoxError::Processing(p) => p.fmt(f),
            SilverfoxError::File(p, e) => write!(
                f,
                "silverfox encountered an i/o error: {}\n(file: {})",
                e,
                p.display()
            ),
            SilverfoxError::Csv(c) => c.fmt(f),
        }
    }
}

impl SilverfoxError {
    pub fn file_error<P: AsRef<Path>>(path: P, error: std::io::Error) -> Self {
        Self::File(path.as_ref().to_path_buf(), error)
    }
}

/// ParseError is thrown during the parsing phase of ledger construction. If silverfox can't parse
/// something, this error type will be thrown.
#[derive(Debug)]
pub struct ParseError {
    pub context: Option<String>,
    pub message: Option<String>,
}

impl Error for ParseError {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.message.is_some() && self.context.is_some() {
            write!(
                f,
                "silverfox couldn't understand the following:\n\n{}\n\n{}",
                self.context.as_ref().unwrap(),
                self.message.as_ref().unwrap(),
            )
        } else if let Some(a) = &self.message {
            write!(f, "silverfox ran across an issue in your journal: {}", a)
        } else if let Some(b) = &self.context {
            write!(
                f,
                "silverfox couldn't understand this:\n\n{}\n\nbut no explanation was provided",
                b
            )
        } else {
            write!(
                f,
                "silverfox couldn't parse something, but no information was provided"
            )
        }
    }
}

/// ValidationError is thrown during the validation phase of ledger construction. If silverfox finds
/// something that's invalid and can't continue with construction, this error type will be thrown.
#[derive(Debug)]
pub struct ValidationError {
    pub context: Option<String>,
    pub message: Option<String>,
}

impl Error for ValidationError {}

impl ValidationError {
    /// Sets the message of the error, returning itself for the convenience of chaining.
    pub fn set_message(mut self, message: &str) -> Self {
        self.message = Some(message.to_string());

        self
    }

    /// Sets the context (chunk) of the error, returning itself for the convenience of chaining.
    pub fn set_context(mut self, context: &str) -> Self {
        self.context = Some(context.to_string());

        self
    }
}

impl Default for ValidationError {
    /// Returns a fresh, blank ValidationError.
    fn default() -> Self {
        ValidationError {
            context: None,
            message: None,
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.message.is_some() && self.context.is_some() {
            write!(
                f,
                "the following is invalid to silverfox:\n\n{}\n\n{}",
                self.context.as_ref().unwrap(),
                self.message.as_ref().unwrap(),
            )
        } else if let Some(a) = &self.message {
            write!(f, "silverfox flagged your journal file as invalid: {}", a)
        } else if let Some(b) = &self.context {
            write!(f, "the following is invalid to silverfox:\n\n{}\n\nbut no further information was provided", b)
        } else {
            write!(
                f,
                "silverfox found something invalid, but no information was provided"
            )
        }
    }
}

#[derive(Debug)]
pub struct ProcessingError {
    pub context: Option<String>,
    pub message: Option<String>,
}

impl Error for ProcessingError {}

impl ProcessingError {
    /// Sets the message of the error, returning itself for the convenience of chaining.
    pub fn set_message(mut self, message: &str) -> Self {
        self.message = Some(message.to_string());

        self
    }

    /// Sets the context (chunk) of the error, returning itself for the convenience of chaining.
    pub fn set_context(mut self, context: &str) -> Self {
        self.context = Some(context.to_string());

        self
    }
}

impl Default for ProcessingError {
    /// Returns a fresh, blank ProcessingError.
    fn default() -> Self {
        ProcessingError {
            context: None,
            message: None,
        }
    }
}

impl fmt::Display for ProcessingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.message.is_some() && self.context.is_some() {
            write!(
                f,
                "looks like your journal is all valid, but this couldn't be processed:\n\n{}\n\n{}",
                self.context.as_ref().unwrap(),
                self.message.as_ref().unwrap(),
            )
        } else if let Some(a) = &self.message {
            write!(
                f,
                "your journal is valid, but silverfox couldn't process this: {}",
                a
            )
        } else if let Some(b) = &self.context {
            write!(f, "your journal is valid, but silverfox couldn't process this:\n\n{}\n\nno further information was provided", b)
        } else {
            write!(f, "your journal is valid, but silverfox couldn't process something. no information was provided. file an issue?")
        }
    }
}
