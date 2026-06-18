use std::num::ParseFloatError;

use miette::{NamedSource, SourceSpan};
use nalgebra::Point3;

use super::{
    atom::Atom,
    element_parser::{parse_element, ParseElementError},
    geometry::Geometry,
    geometry_parse_error::{GeometryAtomLineDiagnostic, GeometryParseError},
};

#[derive(Debug)]
enum AtomLineProblem {
    MissingFields {
        found: usize,
    },
    ExtraField,
    InvalidElement(ParseElementError),
    InvalidCoordinate {
        coordinate_index: usize,
        source: ParseFloatError,
    },
    NonFiniteCoordinate {
        coordinate_index: usize,
    },
}

#[derive(Clone, Copy)]
struct SpannedText<'a> {
    text: &'a str,
    offset: usize,
}

impl SpannedText<'_> {
    fn span(self) -> SourceSpan {
        (self.offset, self.text.len()).into()
    }

    fn end_span(self) -> SourceSpan {
        (self.offset + self.text.len(), 0).into()
    }

    fn trim_end(self, pattern: &[char]) -> Self {
        Self {
            text: self.text.trim_end_matches(pattern),
            offset: self.offset,
        }
    }
}

pub(crate) fn parse_xyz(
    source_name: impl Into<String>,
    source: &str,
) -> Result<Geometry, GeometryParseError> {
    let source_name = source_name.into();
    let lines = source_lines(source);
    let atom_count_line = lines.first().copied().unwrap_or(SpannedText {
        text: "",
        offset: 0,
    });
    let atom_count = atom_count_line
        .text
        .trim()
        .parse::<usize>()
        .map_err(|error| {
            GeometryParseError::invalid_atom_count(
                error,
                source_name.clone(),
                source,
                atom_count_line.span(),
            )
        })?;

    let comment = lines
        .get(1)
        .map(|line| line.text.to_string())
        .unwrap_or_default();
    let mut atoms = Vec::with_capacity(atom_count);
    let mut diagnostics = Vec::new();
    let source_code = NamedSource::new(source_name, source.to_string());

    for atom_index in 0..atom_count {
        let line = lines.get(atom_index + 2).copied().unwrap_or(SpannedText {
            text: "",
            offset: source.len(),
        });
        match read_atom_line(line, atom_index, &source_code) {
            Ok(atom) => atoms.push(atom),
            Err(mut line_diagnostics) => diagnostics.append(&mut line_diagnostics),
        }
    }

    if diagnostics.is_empty() {
        Ok(Geometry::new(comment, atoms))
    } else {
        Err(GeometryParseError::AtomLineErrors {
            count: diagnostics.len(),
            diagnostics,
        })
    }
}

fn read_atom_line(
    line: SpannedText<'_>,
    atom_index: usize,
    source_code: &NamedSource<String>,
) -> Result<Atom, Vec<GeometryAtomLineDiagnostic>> {
    let fields = line_fields(line);
    let mut diagnostics = Vec::new();

    match fields.len() {
        4 => {}
        found if found < 4 => diagnostics.push(atom_line_diagnostic(
            AtomLineProblem::MissingFields { found },
            atom_index,
            line,
            &fields,
            source_code.clone(),
        )),
        _ => diagnostics.push(atom_line_diagnostic(
            AtomLineProblem::ExtraField,
            atom_index,
            line,
            &fields,
            source_code.clone(),
        )),
    }

    let element = fields
        .first()
        .and_then(|field| match parse_element(field.text) {
            Ok(element) => Some(element),
            Err(error) => {
                diagnostics.push(atom_line_diagnostic(
                    AtomLineProblem::InvalidElement(error),
                    atom_index,
                    line,
                    &fields,
                    source_code.clone(),
                ));
                None
            }
        });

    let mut coordinates = [None; 3];
    for coordinate_index in 1..=3 {
        let Some(field) = fields.get(coordinate_index) else {
            continue;
        };
        match field.text.parse::<f64>() {
            Ok(value) if value.is_finite() => coordinates[coordinate_index - 1] = Some(value),
            Ok(_) => diagnostics.push(atom_line_diagnostic(
                AtomLineProblem::NonFiniteCoordinate { coordinate_index },
                atom_index,
                line,
                &fields,
                source_code.clone(),
            )),
            Err(source) => diagnostics.push(atom_line_diagnostic(
                AtomLineProblem::InvalidCoordinate {
                    coordinate_index,
                    source,
                },
                atom_index,
                line,
                &fields,
                source_code.clone(),
            )),
        }
    }

    if diagnostics.is_empty() {
        Ok(Atom::new(
            element.expect("valid atom line has an element"),
            Point3::new(
                coordinates[0].expect("valid atom line has x"),
                coordinates[1].expect("valid atom line has y"),
                coordinates[2].expect("valid atom line has z"),
            ),
        ))
    } else {
        Err(diagnostics)
    }
}

fn source_lines(source: &str) -> Vec<SpannedText<'_>> {
    let mut offset = 0usize;
    let mut lines = Vec::new();

    for raw_line in source.split_inclusive('\n') {
        let line_len = raw_line.len();
        lines.push(
            SpannedText {
                text: raw_line,
                offset,
            }
            .trim_end(&['\r', '\n']),
        );
        offset += line_len;
    }

    if source.is_empty() || !source.ends_with('\n') {
        lines.push(
            SpannedText {
                text: &source[offset..],
                offset,
            }
            .trim_end(&['\r']),
        );
    }

    lines
}

fn line_fields(line: SpannedText<'_>) -> Vec<SpannedText<'_>> {
    let mut fields = Vec::new();
    let mut field_start = None;

    for (index, character) in line.text.char_indices() {
        if character.is_whitespace() {
            if let Some(start) = field_start.take() {
                fields.push(SpannedText {
                    text: &line.text[start..index],
                    offset: line.offset + start,
                });
            }
        } else if field_start.is_none() {
            field_start = Some(index);
        }
    }

    if let Some(start) = field_start {
        fields.push(SpannedText {
            text: &line.text[start..],
            offset: line.offset + start,
        });
    }

    fields
}

fn atom_line_diagnostic(
    problem: AtomLineProblem,
    atom_index: usize,
    line: SpannedText<'_>,
    fields: &[SpannedText<'_>],
    source_code: NamedSource<String>,
) -> GeometryAtomLineDiagnostic {
    let line_number = atom_index + 3;
    let (message, label, span) = match problem {
        AtomLineProblem::MissingFields { found } => {
            let missing = 4 - found;
            (
                format!(
                    "Atom {atom_index} on XYZ line {line_number} is missing {missing} field(s)."
                ),
                "expected exactly: Element x y z",
                line.end_span(),
            )
        }
        AtomLineProblem::ExtraField => (
            format!("Atom {atom_index} on XYZ line {line_number} has extra field(s)."),
            "remove extra field(s); expected exactly: Element x y z",
            fields
                .get(4)
                .map(|field| field.span())
                .unwrap_or_else(|| line.end_span()),
        ),
        AtomLineProblem::InvalidElement(source) => (
            format!("Atom {atom_index} on XYZ line {line_number} has an invalid element: {source}"),
            "invalid element symbol",
            fields
                .first()
                .map(|field| field.span())
                .unwrap_or_else(|| line.span()),
        ),
        AtomLineProblem::InvalidCoordinate {
            coordinate_index,
            source,
        } => {
            let coordinate_name = coordinate_name(coordinate_index);
            (
                format!(
                    "Atom {atom_index} on XYZ line {line_number} has an invalid {coordinate_name} coordinate: {source}"
                ),
                "invalid numeric coordinate",
                fields
                    .get(coordinate_index)
                    .map(|field| field.span())
                    .unwrap_or_else(|| line.span()),
            )
        }
        AtomLineProblem::NonFiniteCoordinate { coordinate_index } => {
            let coordinate_name = coordinate_name(coordinate_index);
            (
                format!(
                    "Atom {atom_index} on XYZ line {line_number} has a non-finite {coordinate_name} coordinate."
                ),
                "coordinate must be finite",
                fields
                    .get(coordinate_index)
                    .map(|field| field.span())
                    .unwrap_or_else(|| line.span()),
            )
        }
    };

    GeometryAtomLineDiagnostic::new(message, label, source_code, span)
}

fn coordinate_name(coordinate_index: usize) -> &'static str {
    ["element", "x", "y", "z"]
        .get(coordinate_index)
        .copied()
        .unwrap_or("coordinate")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geometry_parsing_reports_multiple_atom_line_errors() {
        let xyz_data = "\
3
Broken molecule
Xx 0.0 0.0 0.0
H nope 0.0 0.0
He 0.0 0.0
";

        let error = parse_xyz("broken.xyz", xyz_data).unwrap_err();

        assert!(matches!(
            error,
            GeometryParseError::AtomLineErrors { count: 3, .. }
        ));
        let rendered = format!("{error}");
        assert!(rendered.contains("geometry contains 3 atom line error(s)"));
    }

    #[test]
    fn test_geometry_parsing_reports_atom_count_span() {
        let xyz_data = "\
two
Broken molecule
H 0.0 0.0 0.0
";

        let error = parse_xyz("broken.xyz", xyz_data).unwrap_err();
        let GeometryParseError::ParseNumberOfAtom { span, .. } = error else {
            panic!("expected atom count diagnostic");
        };

        assert_eq!((span.offset(), span.len()), (0, "two".len()));
    }

    #[test]
    fn test_geometry_parsing_reports_precise_atom_line_spans() {
        let xyz_data = "\
4
Broken molecule
Xx 0.0 0.0 0.0
H nope 0.0 0.0
He 0.0 0.0 0.0 extra
Li 0.0 0.0
";

        let error = parse_xyz("broken.xyz", xyz_data).unwrap_err();
        let GeometryParseError::AtomLineErrors { diagnostics, .. } = error else {
            panic!("expected grouped atom line diagnostics");
        };

        let expected_spans = [
            ("Xx", xyz_data.find("Xx").unwrap()),
            ("nope", xyz_data.find("nope").unwrap()),
            ("extra", xyz_data.find("extra").unwrap()),
            (
                "",
                xyz_data.find("Li 0.0 0.0").unwrap() + "Li 0.0 0.0".len(),
            ),
        ];

        for (diagnostic, (text, offset)) in diagnostics.iter().zip(expected_spans) {
            assert_eq!(
                diagnostic.span_offset_and_len(),
                (offset, text.len()),
                "span should point exactly at {text:?}"
            );
        }
    }

    #[test]
    fn test_geometry_parsing_reports_multiple_errors_on_same_atom_line() {
        let xyz_data = "\
1
Broken molecule
Xx nope 0.0 bad
";

        let error = parse_xyz("broken.xyz", xyz_data).unwrap_err();
        let GeometryParseError::AtomLineErrors { count, diagnostics } = error else {
            panic!("expected grouped atom line diagnostics");
        };

        assert_eq!(count, 3);
        let expected_spans = [
            ("Xx", xyz_data.find("Xx").unwrap()),
            ("nope", xyz_data.find("nope").unwrap()),
            ("bad", xyz_data.find("bad").unwrap()),
        ];

        for (diagnostic, (text, offset)) in diagnostics.iter().zip(expected_spans) {
            assert_eq!(
                diagnostic.span_offset_and_len(),
                (offset, text.len()),
                "span should point exactly at {text:?}"
            );
        }
    }

    #[test]
    fn test_geometry_parsing_rejects_non_finite_coordinates() {
        let xyz_data = "\
2
Broken molecule
H NaN 0.0 0.0
He 0.0 inf 0.0
";

        let error = parse_xyz("broken.xyz", xyz_data).unwrap_err();
        let GeometryParseError::AtomLineErrors { count, diagnostics } = error else {
            panic!("expected grouped atom line diagnostics");
        };

        assert_eq!(count, 2);
        let expected_spans = [
            ("NaN", xyz_data.find("NaN").unwrap()),
            ("inf", xyz_data.find("inf").unwrap()),
        ];
        for (diagnostic, (text, offset)) in diagnostics.iter().zip(expected_spans) {
            assert_eq!(diagnostic.span_offset_and_len(), (offset, text.len()));
        }
    }
}
