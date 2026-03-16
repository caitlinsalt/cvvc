use std::{error::Error, fmt::Display};

/// Subtypes of [PathError].
#[derive(Debug)]
pub enum PathErrorKind {
    /// The path given is not a valid path, according to the OS.
    InvalidPath,

    /// The path given is not within a CVVC repository.
    PathOutsideRepo,
}

/// An error has occurred, caused by an invalid path.
#[derive(Debug)]
pub struct PathError {
    /// The "path" which caused the error.
    pub path: String,

    /// The nature of the error caused.
    pub kind: PathErrorKind,
}

impl PathError {
    /// Create a new, owned [PathError] object.
    pub fn new<T: ToString>(path: T, kind: PathErrorKind) -> Self {
        PathError {
            path: path.to_string(),
            kind,
        }
    }
}

impl Display for PathError {
    /// Display a [PathError] error message.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            PathErrorKind::InvalidPath => write!(f, "invalid path '{}'", self.path),
            PathErrorKind::PathOutsideRepo => write!(f, "path '{}' is outside the repo", self.path),
        }
    }
}

impl Error for PathError {}
