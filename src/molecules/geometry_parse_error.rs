use std::{io, num::ParseIntError};

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum GeometryParseError {
    #[error("unable to read XYZ geometry: {0}")]
    #[diagnostic(
        code(rustiq::geometry::io),
        help("Check that the geometry file exists and can be read.")
    )]
    Io(#[from] io::Error),

    #[error("invalid XYZ atom count: {source}")]
    #[diagnostic(
        code(rustiq::geometry::atom_count),
        help("The first XYZ line must be a non-negative integer atom count.")
    )]
    ParseNumberOfAtom {
        #[source]
        source: ParseIntError,
        #[source_code]
        source_code: NamedSource<String>,
        #[label("expected an integer atom count")]
        span: SourceSpan,
    },

    #[error("geometry contains {count} atom line error(s)")]
    #[diagnostic(
        code(rustiq::geometry::atom_lines),
        help("Fix each reported XYZ atom line. Expected format: Element x y z.")
    )]
    AtomLineErrors {
        count: usize,
        #[related]
        diagnostics: Vec<GeometryAtomLineDiagnostic>,
    },
}

impl GeometryParseError {
    pub(crate) fn invalid_atom_count(
        source: ParseIntError,
        source_name: impl Into<String>,
        source_text: &str,
        span: SourceSpan,
    ) -> Self {
        Self::ParseNumberOfAtom {
            source,
            source_code: NamedSource::new(source_name.into(), source_text.to_string()),
            span,
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(rustiq::geometry::invalid_atom_line))]
pub struct GeometryAtomLineDiagnostic {
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("{label}")]
    span: SourceSpan,
    label: String,
}

impl GeometryAtomLineDiagnostic {
    pub(crate) fn new(
        message: impl Into<String>,
        label: impl Into<String>,
        source_code: NamedSource<String>,
        span: SourceSpan,
    ) -> Self {
        Self {
            message: message.into(),
            source_code,
            span,
            label: label.into(),
        }
    }

    #[cfg(test)]
    pub(crate) fn span_offset_and_len(&self) -> (usize, usize) {
        (self.span.offset(), self.span.len())
    }
}
