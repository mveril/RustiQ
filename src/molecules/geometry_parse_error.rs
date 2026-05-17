use std::{
    io,
    num::{ParseFloatError, ParseIntError},
};

use super::element_parser::ParseElementError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GeometryParseError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Atom number parsing error: {0}")]
    ParseNumberOfAtom(#[from] ParseIntError),

    #[error("Unable to parse atom line '{1}' at index {0}")]
    AtomLineShouldHaveFourParts(usize, String),

    #[error("Unable to parse element in atom line '{1}' at index {0}: {2}")]
    AtomLineElementError(usize, String, #[source] ParseElementError),

    #[error("Unable to parse a coordinate of atom at line '{1}' at index {0}: {2}")]
    AtomLineCoordinateError(usize, String, #[source] ParseFloatError),
}
