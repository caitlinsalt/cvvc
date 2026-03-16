use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

/// Raised when a request for one object returns either no objects or multiple candidate objects
#[derive(Debug)]
pub struct FindObjectError {
    /// The potential objects that the request may have been searching for.
    pub candidates: Option<Vec<String>>,
}

impl FindObjectError {
    /// Construct a [`FindObjectError`] that indicates no objects were found.
    pub fn none() -> FindObjectError {
        FindObjectError { candidates: None }
    }

    /// Construct a [`FindObjectError`] that indicates multiple objects were found, listing them.
    ///
    /// If an empty slice is passed, it is equivalent to calling [`FindObjectError::none`].
    pub fn some(candidates: &[String]) -> FindObjectError {
        FindObjectError {
            candidates: Some(
                candidates
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<String>>(),
            ),
        }
    }
}

impl Display for FindObjectError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.candidates {
            Some(_) => write!(f, "multiple objects found"),
            None => write!(f, "no objects found"),
        }
    }
}

impl Error for FindObjectError {}

/// Raised when an object ID is invalid or missing, for example when a commit does not contain a tree reference.
#[derive(Debug)]
pub struct InvalidObjectIdError {}

impl Display for InvalidObjectIdError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "invalid or missing object ID")
    }
}

impl Error for InvalidObjectIdError {}
