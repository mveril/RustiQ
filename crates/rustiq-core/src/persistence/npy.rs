use std::io::{Read, Seek, Write};

use nalgebra::DMatrix;
use npyz::{DType, NpyFile, TypeChar, WriterBuilder};

use crate::eri::CompactEri;

use super::PersistenceError;

/// Conversion between scientific values and an NPY byte stream.
pub trait NpyConvert: Sized {
    type Shape;

    /// Converts a parsed NPY array into the scientific value.
    fn from_npy<R: Read>(npy: NpyFile<R>) -> Result<Self, PersistenceError>;

    /// Reads from any byte stream, including an in-memory buffer.
    fn read_npy(reader: impl Read) -> Result<Self, PersistenceError> {
        Self::from_npy(NpyFile::new(reader).map_err(PersistenceError::NpyRead)?)
    }

    fn npy_shape(shape: Self::Shape) -> Result<Vec<u64>, PersistenceError>;

    /// Checks a parsed header before decoding any array values.
    fn try_from_npy_with_shape<R: Read>(
        npy: NpyFile<R>,
        shape: Self::Shape,
    ) -> Result<Self, PersistenceError> {
        let expected = Self::npy_shape(shape)?;
        let actual = npy.shape().to_vec();
        if actual != expected {
            return Err(PersistenceError::InvalidNpyShape { expected, actual });
        }
        Self::from_npy(npy)
    }

    /// Parses a stream once, checks its shape, then decodes its values.
    fn try_read_with_shape(
        reader: impl Read,
        shape: Self::Shape,
    ) -> Result<Self, PersistenceError> {
        Self::try_from_npy_with_shape(
            NpyFile::new(reader).map_err(PersistenceError::NpyRead)?,
            shape,
        )
    }

    fn write_npy(&self, writer: impl Write) -> Result<(), PersistenceError>;
}

impl NpyConvert for CompactEri {
    type Shape = usize;

    fn write_npy(&self, writer: impl Write) -> Result<(), PersistenceError> {
        write_compact_eri(writer, self)
    }

    fn from_npy<R: Read>(npy: NpyFile<R>) -> Result<Self, PersistenceError> {
        decode_compact_eri(npy, None)
    }

    fn npy_shape(basis_functions: usize) -> Result<Vec<u64>, PersistenceError> {
        let length = CompactEri::checked_storage_len(basis_functions).ok_or_else(|| {
            PersistenceError::InvalidArtifact(
                "basis-function count overflows compact ERI storage".into(),
            )
        })?;
        Ok(vec![u64::try_from(length).map_err(|_| {
            PersistenceError::InvalidArtifact("compact ERI length exceeds NPY limits".into())
        })?])
    }
}

impl NpyConvert for DMatrix<f64> {
    type Shape = (usize, usize);

    fn from_npy<R: Read>(npy: NpyFile<R>) -> Result<Self, PersistenceError> {
        decode_dmatrix(npy)
    }

    fn npy_shape((rows, columns): (usize, usize)) -> Result<Vec<u64>, PersistenceError> {
        let rows = u64::try_from(rows).map_err(|_| {
            PersistenceError::InvalidArtifact("matrix rows exceed NPY limits".into())
        })?;
        let columns = u64::try_from(columns).map_err(|_| {
            PersistenceError::InvalidArtifact("matrix columns exceed NPY limits".into())
        })?;
        Ok(vec![rows, columns])
    }

    fn write_npy(&self, writer: impl Write) -> Result<(), PersistenceError> {
        let shape = [self.nrows() as u64, self.ncols() as u64];
        let mut writer = npyz::WriteOptions::new()
            .default_dtype()
            .order(npyz::Order::Fortran)
            .shape(&shape)
            .writer(writer)
            .begin_nd()
            .map_err(PersistenceError::NpyWrite)?;
        writer
            .extend(self.as_slice().iter().copied())
            .map_err(PersistenceError::NpyWrite)?;
        writer.finish().map_err(PersistenceError::NpyWrite)
    }
}

pub(crate) fn write_compact_eri(
    writer: impl Write,
    eri: &CompactEri,
) -> Result<(), PersistenceError> {
    let shape = [eri.len() as u64];
    let mut writer = npyz::WriteOptions::new()
        .default_dtype()
        .shape(&shape)
        .writer(writer)
        .begin_nd()
        .map_err(PersistenceError::NpyWrite)?;
    writer
        .extend(eri.ordered_values().iter().copied())
        .map_err(PersistenceError::NpyWrite)?;
    writer.finish().map_err(PersistenceError::NpyWrite)
}

pub(crate) fn read_compact_eri(
    reader: impl Read,
    basis_functions: usize,
) -> Result<CompactEri, PersistenceError> {
    decode_compact_eri(
        NpyFile::new(reader).map_err(PersistenceError::NpyRead)?,
        Some(basis_functions),
    )
}

fn decode_compact_eri<R: Read>(
    npy: NpyFile<R>,
    basis_functions: Option<usize>,
) -> Result<CompactEri, PersistenceError> {
    if npy.shape().len() != 1 {
        return Err(PersistenceError::InvalidEriShape(npy.shape().to_vec()));
    }
    let actual = usize::try_from(npy.shape()[0])
        .map_err(|_| PersistenceError::InvalidEriShape(npy.shape().to_vec()))?;
    let basis_functions = if let Some(basis_functions) = basis_functions {
        let expected = CompactEri::checked_storage_len(basis_functions).ok_or(
            PersistenceError::InvalidValueCount {
                basis_functions,
                expected: 0,
                actual,
            },
        )?;
        if actual != expected {
            return Err(PersistenceError::InvalidValueCount {
                basis_functions,
                expected,
                actual,
            });
        }
        basis_functions
    } else {
        basis_functions_for_len(actual)
            .ok_or_else(|| PersistenceError::InvalidEriShape(npy.shape().to_vec()))?
    };
    let values = npy.into_vec::<f64>().map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            PersistenceError::InvalidDtype(error.to_string())
        } else {
            PersistenceError::NpyRead(error)
        }
    })?;
    CompactEri::from_ordered_values(basis_functions, values).map_err(|error| match error {
        crate::eri::CompactEriBuildError::InvalidLength { expected, actual } => {
            PersistenceError::InvalidValueCount {
                basis_functions,
                expected,
                actual,
            }
        }
    })
}

fn basis_functions_for_len(length: usize) -> Option<usize> {
    let (mut low, mut high) = (0, usize::MAX);
    while low <= high {
        let middle = low + (high - low) / 2;
        match CompactEri::checked_storage_len(middle) {
            Some(actual) if actual == length => return Some(middle),
            Some(actual) if actual < length => low = middle + 1,
            _ => high = middle.checked_sub(1)?,
        }
    }
    None
}

pub(crate) fn validate_compact_eri_header(
    reader: impl Read + Seek,
    basis_functions: usize,
    file_size: u64,
) -> bool {
    let Some(expected) = CompactEri::checked_storage_len(basis_functions) else {
        return false;
    };
    let mut reader = std::io::BufReader::new(reader);
    let npy = match NpyFile::new(&mut reader) {
        Ok(npy) => npy,
        Err(_) => return false,
    };
    let dtype = npy.dtype();
    let valid_dtype = matches!(
        dtype,
        DType::Plain(type_str)
            if type_str.type_char() == TypeChar::Float
                && type_str.size_field() == 8
                && matches!(
                    type_str.endianness(),
                    npyz::Endianness::Little | npyz::Endianness::Big
                )
    );
    let Some(expected) = u64::try_from(expected).ok() else {
        return false;
    };
    let valid_shape = npy.shape() == [expected];
    let data_offset = match reader.stream_position() {
        Ok(position) => position,
        Err(_) => return false,
    };
    valid_dtype
        && valid_shape
        && data_offset
            .checked_add(expected.saturating_mul(8))
            .is_some_and(|end| end == file_size)
}

pub(crate) fn read_dmatrix(reader: impl Read) -> Result<DMatrix<f64>, PersistenceError> {
    decode_dmatrix(NpyFile::new(reader).map_err(PersistenceError::NpyRead)?)
}

fn decode_dmatrix<R: Read>(npy: NpyFile<R>) -> Result<DMatrix<f64>, PersistenceError> {
    if npy.shape().len() != 2 {
        return Err(PersistenceError::InvalidMatrixShape(npy.shape().to_vec()));
    }
    let rows = usize::try_from(npy.shape()[0])
        .map_err(|_| PersistenceError::InvalidMatrixShape(npy.shape().to_vec()))?;
    let columns = usize::try_from(npy.shape()[1])
        .map_err(|_| PersistenceError::InvalidMatrixShape(npy.shape().to_vec()))?;
    rows.checked_mul(columns)
        .ok_or_else(|| PersistenceError::InvalidMatrixShape(npy.shape().to_vec()))?;
    let order = npy.order();
    let values = npy.into_vec::<f64>().map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            PersistenceError::InvalidDtype(error.to_string())
        } else {
            PersistenceError::NpyRead(error)
        }
    })?;
    Ok(matrix_from_npy_values(rows, columns, order, values))
}

fn matrix_from_npy_values(
    rows: usize,
    columns: usize,
    order: npyz::Order,
    values: Vec<f64>,
) -> DMatrix<f64> {
    match order {
        npyz::Order::C => DMatrix::from_row_slice(rows, columns, &values),
        npyz::Order::Fortran => DMatrix::from_vec(rows, columns, values),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eri::index::EriIndex;
    use std::fs::File;

    #[test]
    fn compact_eri_round_trips_through_npy_in_stable_order() {
        let basis_functions = 4;
        let mut source = CompactEri::Zeroed(basis_functions);
        for index in 0..source.len() {
            source[EriIndex(index)] = index as f64 + 0.5;
        }
        let mut bytes = Vec::new();
        write_compact_eri(&mut bytes, &source).unwrap();
        let restored = read_compact_eri(bytes.as_slice(), basis_functions).unwrap();
        assert_eq!(restored.ordered_values(), source.ordered_values());
        let inferred = CompactEri::read_npy(bytes.as_slice()).unwrap();
        assert_eq!(inferred.ordered_values(), source.ordered_values());
        assert_eq!(
            CompactEri::from_npy(NpyFile::new(bytes.as_slice()).unwrap())
                .unwrap()
                .ordered_values(),
            source.ordered_values()
        );
        assert_eq!(
            CompactEri::try_read_with_shape(bytes.as_slice(), basis_functions)
                .unwrap()
                .ordered_values(),
            source.ordered_values()
        );
        assert!(matches!(
            CompactEri::try_read_with_shape(bytes.as_slice(), basis_functions - 1),
            Err(PersistenceError::InvalidNpyShape { .. })
        ));
    }

    #[test]
    fn read_dmatrix_converts_c_order_npy_to_nalgebra_layout() {
        let shape = [2, 3];
        let values = [1.0_f64, 2.0, 3.0, 4.0, 5.0, 6.0];
        let mut bytes = Vec::new();
        let mut writer = npyz::WriteOptions::new()
            .default_dtype()
            .shape(&shape)
            .writer(&mut bytes)
            .begin_nd()
            .unwrap();
        writer.extend(values).unwrap();
        writer.finish().unwrap();

        let matrix = read_dmatrix(bytes.as_slice()).unwrap();

        assert_eq!(matrix.nrows(), 2);
        assert_eq!(matrix.ncols(), 3);
        assert_eq!(matrix[(0, 0)], 1.0);
        assert_eq!(matrix[(0, 1)], 2.0);
        assert_eq!(matrix[(0, 2)], 3.0);
        assert_eq!(matrix[(1, 0)], 4.0);
        assert_eq!(matrix[(1, 1)], 5.0);
        assert_eq!(matrix[(1, 2)], 6.0);
    }

    #[test]
    fn dmatrix_trait_writes_fortran_order_and_reads_both_orders() {
        let matrix = DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let mut bytes = Vec::new();
        matrix.write_npy(&mut bytes).unwrap();
        let npy = NpyFile::new(bytes.as_slice()).unwrap();
        assert_eq!(npy.order(), npyz::Order::Fortran);
        assert_eq!(npy.shape(), &[2, 3]);
        assert_eq!(npy.into_vec::<f64>().unwrap(), matrix.as_slice());
        assert_eq!(DMatrix::<f64>::read_npy(bytes.as_slice()).unwrap(), matrix);
        assert_eq!(
            DMatrix::<f64>::from_npy(NpyFile::new(bytes.as_slice()).unwrap()).unwrap(),
            matrix
        );
        assert_eq!(
            DMatrix::<f64>::try_from_npy_with_shape(
                NpyFile::new(bytes.as_slice()).unwrap(),
                (2, 3)
            )
            .unwrap(),
            matrix
        );
        assert_eq!(
            DMatrix::<f64>::try_read_with_shape(bytes.as_slice(), (2, 3)).unwrap(),
            matrix
        );
        assert!(matches!(
            DMatrix::<f64>::try_read_with_shape(bytes.as_slice(), (3, 2)),
            Err(PersistenceError::InvalidNpyShape { .. })
        ));
        let truncated = &bytes[..bytes.len() - 8];
        assert!(matches!(
            DMatrix::<f64>::try_read_with_shape(truncated, (3, 2)),
            Err(PersistenceError::InvalidNpyShape { .. })
        ));
        assert!(matches!(
            DMatrix::<f64>::try_read_with_shape(truncated, (2, 3)),
            Err(PersistenceError::NpyRead(_))
        ));

        let mut c_bytes = Vec::new();
        let mut writer = npyz::WriteOptions::new()
            .default_dtype()
            .shape(&[2, 3])
            .writer(&mut c_bytes)
            .begin_nd()
            .unwrap();
        writer.extend([1.0_f64, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        writer.finish().unwrap();
        assert_eq!(
            DMatrix::<f64>::read_npy(c_bytes.as_slice()).unwrap(),
            matrix
        );
    }

    #[test]
    fn rejects_wrong_value_count() {
        let mut bytes = Vec::new();
        let shape = [2];
        let mut writer = npyz::WriteOptions::new()
            .default_dtype()
            .shape(&shape)
            .writer(&mut bytes)
            .begin_nd()
            .unwrap();
        writer.extend([1.0_f64, 2.0]).unwrap();
        writer.finish().unwrap();
        assert!(matches!(
            read_compact_eri(bytes.as_slice(), 2),
            Err(PersistenceError::InvalidValueCount { .. })
        ));
    }

    #[test]
    fn rejects_wrong_dtype() {
        let mut bytes = Vec::new();
        let shape = [1];
        let mut writer = npyz::WriteOptions::new()
            .default_dtype()
            .shape(&shape)
            .writer(&mut bytes)
            .begin_nd()
            .unwrap();
        writer.push(&7_i32).unwrap();
        writer.finish().unwrap();
        assert!(matches!(
            read_compact_eri(bytes.as_slice(), 1),
            Err(PersistenceError::InvalidDtype(_))
        ));
    }

    #[test]
    fn rejects_truncated_payload() {
        let source = CompactEri::Zeroed(3);
        let mut bytes = Vec::new();
        write_compact_eri(&mut bytes, &source).unwrap();
        bytes.truncate(bytes.len() - 1);
        assert!(matches!(
            read_compact_eri(bytes.as_slice(), 3),
            Err(PersistenceError::NpyRead(_))
        ));
    }

    #[test]
    fn reads_python_numpy_fixture() {
        let hex = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/persistence/ao-eri-python-v1.npy.hex"
        ));
        let (pairs, remainder) = hex.trim().as_bytes().as_chunks::<2>();
        assert!(remainder.is_empty());
        let bytes: Vec<u8> = pairs
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let eri = read_compact_eri(bytes.as_slice(), 2).unwrap();
        assert_eq!(eri.ordered_values(), &[0.5, 1.5, 2.5, 3.5, 4.5, 5.5]);
    }

    #[test]
    fn reads_big_endian_python_numpy_fixture() {
        let hex = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/persistence/ao-eri-python-big-endian-v1.npy.hex"
        ));
        let (pairs, remainder) = hex.trim().as_bytes().as_chunks::<2>();
        assert!(remainder.is_empty());
        let bytes: Vec<u8> = pairs
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let eri = read_compact_eri(bytes.as_slice(), 2).unwrap();
        assert_eq!(eri.ordered_values(), &[0.5, 1.5, 2.5, 3.5, 4.5, 5.5]);
    }

    #[test]
    #[ignore = "called by the NumPy interoperability test"]
    fn writes_npy_for_numpy_interoperability() {
        let output = std::env::var_os("RUSTIQ_NPY_TEST_OUTPUT")
            .expect("RUSTIQ_NPY_TEST_OUTPUT must name the NPY output file");
        let mut source = CompactEri::Zeroed(2);
        for index in 0..source.len() {
            source[EriIndex(index)] = index as f64 + 0.5;
        }

        write_compact_eri(File::create(output).unwrap(), &source).unwrap();
    }
}
