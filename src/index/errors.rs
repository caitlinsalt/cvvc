//! Error structs for errors specific to index parsing.

use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

/// The reasons that an index entry may be invalid.
#[derive(Debug)]
pub enum InvalidIndexEntryKind {
    /// The entry was too short to be properly parsed.
    TooShort,

    /// The file mode was not one of the valid file values for an index entry.
    UnexpectedMode(u16),

    /// The file permissions field was not one of the valid permissions value for an index entry.
    UnexpectedPermissions(u16),

    /// The parser ran out of data before finding a NUL value to terminate the entry name.
    NameNotNullTerminated,

    /// The index timestamp value could not be parsed to a valid timestamp.
    UnparseableTimestamp(u32, u32),
}

/// An error which occurs when the index parser cannot parse an individual index entry.
#[derive(Debug)]
pub struct InvalidIndexEntryError {
    /// The reason the index entry could not be parsed.
    pub error_kind: InvalidIndexEntryKind,
}

impl Display for InvalidIndexEntryError {
    /// Converts an [`InvalidIndexEntryError`] to a human-readable string.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let msg = match self.error_kind {
            InvalidIndexEntryKind::TooShort => String::from("not enough data"),
            InvalidIndexEntryKind::UnexpectedMode(m) => format!("unexpected mode {m:#04b}"),
            InvalidIndexEntryKind::UnexpectedPermissions(p) => {
                format!("unexpected permissions {p:#04o}")
            }
            InvalidIndexEntryKind::NameNotNullTerminated => {
                String::from("name not null-terminated")
            }
            InvalidIndexEntryKind::UnparseableTimestamp(s, ns) => {
                format!("unparseable timestamp {s}.{ns}")
            }
        };
        write!(f, "invalid index entry: {msg}")
    }
}

impl Error for InvalidIndexEntryError {}

/// The reasons that an entire index may be invalid or unparseable.
#[derive(Debug)]
pub enum InvalidIndexKind {
    /// The index was too short to be properly parsed.  This implies the parser ran out of data before reaching the
    /// first index entry.  If the parser has beg
    TooShort,

    /// The header indicating that this file is an index was missing.
    MissingMagic,

    /// The index's version number is not supported by CVVC.
    UnsupportedVersion(u32),

    /// One or more of the entries in the index could not be parsed.
    InvalidEntry(InvalidIndexEntryError),
}

/// An error occurred when parsing an index.  This may have occurred due to an issue with an
/// individual entry, or with the index as a whole.
#[derive(Debug)]
pub struct InvalidIndexError {
    pub error_kind: InvalidIndexKind,
}

impl Display for InvalidIndexError {
    /// Converts an [`InvalidIndexError`] to a human-readable string.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let msg = match &self.error_kind {
            InvalidIndexKind::TooShort => String::from("not enough data"),
            InvalidIndexKind::MissingMagic => String::from("missing magic number"),
            InvalidIndexKind::UnsupportedVersion(v) => format!("unsupported index version {v}"),
            InvalidIndexKind::InvalidEntry(e) => format!("{e}"),
        };
        write!(f, "invalid index: {msg}")
    }
}

impl Error for InvalidIndexError {}
