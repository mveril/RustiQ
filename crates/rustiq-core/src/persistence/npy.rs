use std::io::{Read, Seek, Write};

#[cfg(test)]
use nalgebra::{DMatrix, DVector};
mod nalgebra_impl;
pub(crate) use nalgebra_impl::read_dmatrix;
use npyz::{DType, NpyFile, TypeChar, WriterBuilder};

use crate::eri::CompactEri;

use super::NpyError;

/// Conversion between scientific values and an NPY byte stream.
pub(crate) trait NpyConvert: Sized {
    type Shape: Copy;
    type NpyShape: AsRef<[u64]>;

    fn npy_shape(shape: Self::Shape) -> Result<Self::NpyShape, NpyError>;

    fn decode_with_shape<R: Read>(npy: NpyFile<R>, shape: Self::Shape) -> Result<Self, NpyError>;

    /// Checks a parsed header before decoding any array values.
    fn try_from_npy_with_shape<R: Read>(
        npy: NpyFile<R>,
        shape: Self::Shape,
    ) -> Result<Self, NpyError> {
        let expected = Self::npy_shape(shape)?;
        check_npy_shape(expected.as_ref(), npy.shape())?;
        Self::decode_with_shape(npy, shape)
    }

    /// Parses a stream once, checks its shape, then decodes its values.
    fn try_read_with_shape(reader: impl Read, shape: Self::Shape) -> Result<Self, NpyError> {
        Self::try_from_npy_with_shape(NpyFile::new(reader).map_err(NpyError::Read)?, shape)
    }

    fn write_npy(&self, writer: impl Write) -> Result<(), NpyError>;
}

/// Checks that two NPY headers declare identical ranks and axis lengths.
/// Reads headers only; values, dtype compatibility, and payload integrity are not checked.
///
/// # Errors
/// Returns a read error for invalid headers, or `InvalidShape` for different shapes.
pub fn validate_matching_npy_shapes(
    alpha: impl Read,
    beta: impl Read,
) -> Result<Box<[u64]>, NpyError> {
    let alpha = NpyFile::new(alpha).map_err(NpyError::Read)?;
    let beta = NpyFile::new(beta).map_err(NpyError::Read)?;
    check_npy_shape(alpha.shape(), beta.shape())?;
    Ok(alpha.shape().to_vec().into_boxed_slice())
}

fn check_npy_shape(expected: &[u64], actual: &[u64]) -> Result<(), NpyError> {
    if expected != actual {
        return Err(NpyError::InvalidShape {
            expected: expected.into(),
            actual: actual.into(),
        });
    }
    Ok(())
}

impl NpyConvert for CompactEri {
    type Shape = usize;
    type NpyShape = [u64; 1];

    fn npy_shape(basis_functions: usize) -> Result<Self::NpyShape, NpyError> {
        let length =
            CompactEri::checked_storage_len(basis_functions).ok_or(NpyError::DimensionOverflow)?;
        Ok([u64::try_from(length).map_err(|_| NpyError::DimensionOverflow)?])
    }

    fn decode_with_shape<R: Read>(
        npy: NpyFile<R>,
        basis_functions: usize,
    ) -> Result<Self, NpyError> {
        decode_compact_eri(npy, basis_functions)
    }

    fn write_npy(&self, writer: impl Write) -> Result<(), NpyError> {
        let shape = [u64::try_from(self.len()).map_err(|_| NpyError::DimensionOverflow)?];
        let mut writer = npyz::WriteOptions::new()
            .default_dtype()
            .shape(&shape)
            .writer(writer)
            .begin_nd()
            .map_err(NpyError::Write)?;
        writer
            .extend(self.ordered_values().iter().copied())
            .map_err(NpyError::Write)?;
        writer.finish().map_err(NpyError::Write)
    }
}

pub(crate) fn write_compact_eri(writer: impl Write, eri: &CompactEri) -> Result<(), NpyError> {
    eri.write_npy(writer)
}

pub(crate) fn read_compact_eri(
    reader: impl Read,
    basis_functions: usize,
) -> Result<CompactEri, NpyError> {
    CompactEri::try_read_with_shape(reader, basis_functions)
}

/// Validates the small NPY header and exact payload length before allocating values.
pub(crate) fn checked_eri_prefix(
    reader: &mut dyn Read,
    basis_functions: usize,
    file_size: u64,
) -> Result<Vec<u8>, NpyError> {
    let mut prefix = [0_u8; 8];
    reader.read_exact(&mut prefix).map_err(NpyError::Read)?;
    if &prefix[..6] != b"\x93NUMPY" || !matches!((prefix[6], prefix[7]), (1..=3, 0)) {
        return Err(NpyError::Read(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid NPY prefix",
        )));
    }
    let length_bytes = if prefix[6] == 1 { 2 } else { 4 };
    let mut length = [0_u8; 4];
    reader
        .read_exact(&mut length[..length_bytes])
        .map_err(NpyError::Read)?;
    let header_len =
        usize::try_from(u32::from_le_bytes(length)).map_err(|_| NpyError::DimensionOverflow)?;
    if header_len > 65536 {
        return Err(NpyError::Read(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "NPY header exceeds 64 KiB",
        )));
    }
    let mut header = Vec::with_capacity(8 + length_bytes + header_len);
    header.extend_from_slice(&prefix);
    header.extend_from_slice(&length[..length_bytes]);
    header.resize(8 + length_bytes + header_len, 0);
    reader
        .read_exact(&mut header[8 + length_bytes..])
        .map_err(NpyError::Read)?;
    if !validate_compact_eri_header(std::io::Cursor::new(&header), basis_functions, file_size) {
        return Err(NpyError::Read(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "NPY dtype, shape or payload length is invalid",
        )));
    }
    Ok(header)
}

pub(crate) fn read_checked_compact_eri(
    reader: &mut dyn Read,
    basis_functions: usize,
    file_size: u64,
) -> Result<CompactEri, NpyError> {
    let header = checked_eri_prefix(reader, basis_functions, file_size)?;
    read_compact_eri(std::io::Cursor::new(header).chain(reader), basis_functions)
}

fn decode_compact_eri<R: Read>(
    npy: NpyFile<R>,
    basis_functions: usize,
) -> Result<CompactEri, NpyError> {
    if npy.shape().len() != 1 {
        return Err(NpyError::InvalidEriShape(
            npy.shape().to_vec().into_boxed_slice(),
        ));
    }
    let actual = usize::try_from(npy.shape()[0])
        .map_err(|_| NpyError::InvalidEriShape(npy.shape().to_vec().into_boxed_slice()))?;
    let expected =
        CompactEri::checked_storage_len(basis_functions).ok_or(NpyError::InvalidValueCount {
            basis_functions,
            expected: 0,
            actual,
        })?;
    if actual != expected {
        return Err(NpyError::InvalidValueCount {
            basis_functions,
            expected,
            actual,
        });
    }

    let values = npy
        .data::<f64>()
        .map_err(|error| NpyError::InvalidDtype(error.to_string()))?;
    let mut values = ExactNpyValues::new(values, expected);
    let eri =
        CompactEri::from_ordered_values_iter(basis_functions, &mut values).map_err(|error| {
            match error {
                crate::eri::CompactEriBuildError::InvalidLength { expected, actual } => {
                    NpyError::InvalidValueCount {
                        basis_functions,
                        expected,
                        actual,
                    }
                }
            }
        })?;

    if let Some(error) = values.take_error() {
        return Err(map_npy_value_read_error(error));
    }
    Ok(eri)
}

struct ExactNpyValues<T: npyz::Deserialize + Default, R: Read> {
    reader: npyz::NpyReader<T, R>,
    remaining: usize,
    error: Option<std::io::Error>,
}

impl<T: npyz::Deserialize + Default, R: Read> ExactNpyValues<T, R> {
    fn new(reader: npyz::NpyReader<T, R>, length: usize) -> Self {
        Self {
            reader,
            remaining: length,
            error: None,
        }
    }

    fn take_error(&mut self) -> Option<std::io::Error> {
        self.error.take()
    }
}

impl<T: npyz::Deserialize + Default, R: Read> Iterator for ExactNpyValues<T, R> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;

        match self.reader.next() {
            Some(Ok(value)) => Some(value),
            Some(Err(error)) => {
                if self.error.is_none() {
                    self.error = Some(error);
                }
                Some(T::default())
            }
            None => {
                if self.error.is_none() {
                    self.error = Some(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "NPY data ended before its declared element count",
                    ));
                }
                Some(T::default())
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T: npyz::Deserialize + Default, R: Read> ExactSizeIterator for ExactNpyValues<T, R> {}

fn map_npy_value_read_error(error: std::io::Error) -> NpyError {
    if error.kind() == std::io::ErrorKind::InvalidData {
        NpyError::InvalidDtype(error.to_string())
    } else {
        NpyError::Read(error)
    }
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
    let Ok(npy) = NpyFile::new(&mut reader) else {
        return false;
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
    let Ok(data_offset) = reader.stream_position() else {
        return false;
    };
    valid_dtype
        && valid_shape
        && data_offset
            .checked_add(expected.saturating_mul(8))
            .is_some_and(|end| end == file_size)
}

#[cfg(test)]
#[allow(
    unknown_lints,
    reason = "assert_is_empty is only available starting with Clippy 1.99"
)]
#[allow(
    clippy::assert_is_empty,
    reason = "Idiomatic is_empty assertions express the test intent without typed empty collections"
)]
mod tests {
    use super::*;
    use crate::eri::index::EriIndex;
    use std::fs::File;

    #[test]
    fn matching_shapes_checks_rank_and_each_axis_without_reading_values() {
        let mut matrix = Vec::new();
        DMatrix::<f64>::zeros(2, 3).write_npy(&mut matrix).unwrap();
        let mut same = Vec::new();
        DMatrix::<f64>::zeros(2, 3).write_npy(&mut same).unwrap();
        assert_eq!(
            &*validate_matching_npy_shapes(matrix.as_slice(), same.as_slice()).unwrap(),
            &[2, 3]
        );
        let mut transposed = Vec::new();
        DMatrix::<f64>::zeros(3, 2)
            .write_npy(&mut transposed)
            .unwrap();
        assert!(matches!(
            validate_matching_npy_shapes(matrix.as_slice(), transposed.as_slice()),
            Err(NpyError::InvalidShape { .. })
        ));
        let mut vector = Vec::new();
        DVector::<f64>::zeros(6).write_npy(&mut vector).unwrap();
        assert!(matches!(
            validate_matching_npy_shapes(matrix.as_slice(), vector.as_slice()),
            Err(NpyError::InvalidShape { .. })
        ));
        assert!(matches!(
            validate_matching_npy_shapes(&b"invalid"[..], vector.as_slice()),
            Err(NpyError::Read(_))
        ));
    }

    #[test]
    #[allow(
        clippy::cast_precision_loss,
        reason = "Fixture dimensions and quadrature orders are small enough to be represented exactly in f64"
    )]
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
        assert_eq!(
            CompactEri::try_read_with_shape(bytes.as_slice(), basis_functions)
                .unwrap()
                .ordered_values(),
            source.ordered_values()
        );
        assert!(matches!(
            CompactEri::try_read_with_shape(bytes.as_slice(), basis_functions - 1),
            Err(NpyError::InvalidShape { .. })
        ));
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "Changing the NPY memory layout must preserve the stored values exactly"
    )]
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
        assert_eq!(read_dmatrix(bytes.as_slice()).unwrap(), matrix);
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
            Err(NpyError::InvalidShape { .. })
        ));
        let truncated = &bytes[..bytes.len() - 8];
        assert!(matches!(
            DMatrix::<f64>::try_read_with_shape(truncated, (3, 2)),
            Err(NpyError::InvalidShape { .. })
        ));
        assert!(matches!(
            DMatrix::<f64>::try_read_with_shape(truncated, (2, 3)),
            Err(NpyError::Read(_))
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
        assert_eq!(read_dmatrix(c_bytes.as_slice()).unwrap(), matrix);
    }

    #[test]
    fn reads_python_numpy_matrix_fixtures_in_c_and_fortran_order() {
        for hex in [
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/data/persistence/dmatrix-python-c-v1.npy.hex"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/data/persistence/dmatrix-python-fortran-v1.npy.hex"
            )),
        ] {
            let (pairs, remainder) = hex.trim().as_bytes().as_chunks::<2>();
            assert!(remainder.is_empty());
            let bytes: Vec<u8> = pairs
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();

            assert_eq!(
                read_dmatrix(bytes.as_slice()).unwrap(),
                DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
            );
        }
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
            Err(NpyError::InvalidShape { .. })
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
            Err(NpyError::InvalidDtype(_))
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
            Err(NpyError::Read(_))
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
    #[allow(
        clippy::cast_precision_loss,
        reason = "Fixture dimensions and quadrature orders are small enough to be represented exactly in f64"
    )]
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

#[cfg(test)]
mod vector_shapes {
    use super::*;

    #[test]
    fn generic_matrix_supports_static_and_dynamic_dimensions() {
        let source = nalgebra::Matrix2x3::new(1., 2., 3., 4., 5., 6.);
        let mut bytes = Vec::new();
        source.write_npy(&mut bytes).unwrap();
        assert_eq!(
            nalgebra::Matrix2x3::try_read_with_shape(bytes.as_slice(), (2, 3)).unwrap(),
            source
        );
        assert_eq!(
            DMatrix::try_read_with_shape(bytes.as_slice(), (2, 3))
                .unwrap()
                .as_slice(),
            source.as_slice()
        );
        assert!(nalgebra::Matrix3x2::try_read_with_shape(bytes.as_slice(), (2, 3)).is_err());
        let source = nalgebra::Vector3::new(1., 2., 3.);
        bytes.clear();
        source.write_npy(&mut bytes).unwrap();
        assert_eq!(
            npyz::NpyFile::new(bytes.as_slice()).unwrap().shape(),
            &[3, 1]
        );
        assert!(DMatrix::try_read_with_shape(bytes.as_slice(), (3, 1)).is_ok());
        assert!(DMatrix::try_read_with_shape(bytes.as_slice(), (1, 3)).is_err());
        assert!(nalgebra::RowVector3::try_read_with_shape(bytes.as_slice(), (1, 3)).is_err());
    }

    #[test]
    fn accepts_vector_axes_but_rejects_matrices_before_decoding() {
        for shape in [&[3][..], &[3, 1], &[1, 3], &[2, 2], &[1, 1, 3], &[4, 1]] {
            for order in [npyz::Order::C, npyz::Order::Fortran] {
                let mut bytes = Vec::new();
                let mut writer = npyz::WriteOptions::new()
                    .default_dtype()
                    .shape(shape)
                    .order(order)
                    .writer(&mut bytes)
                    .begin_nd()
                    .unwrap();
                writer
                    .extend(
                        [1.0_f64, 2., 3., 4.]
                            .into_iter()
                            .take(usize::try_from(shape.iter().product::<u64>()).unwrap()),
                    )
                    .unwrap();
                writer.finish().unwrap();
                for expected in [(3, 1), (1, 3)] {
                    let dynamic = DMatrix::<f64>::try_read_with_shape(bytes.as_slice(), expected);
                    if shape == [expected.0 as u64, expected.1 as u64] {
                        let dynamic = dynamic.unwrap();
                        assert_eq!(dynamic.shape(), expected);
                        assert_eq!(dynamic.as_slice(), &[1., 2., 3.]);
                    } else {
                        assert!(matches!(dynamic, Err(NpyError::InvalidShape { .. })));
                    }
                }
                let row =
                    nalgebra::RowDVector::<f64>::try_read_with_shape(bytes.as_slice(), (1, 3));
                let static_column =
                    nalgebra::Vector3::<f64>::try_read_with_shape(bytes.as_slice(), (3, 1));
                let static_row =
                    nalgebra::RowVector3::<f64>::try_read_with_shape(bytes.as_slice(), (1, 3));
                assert_eq!(row.is_ok(), shape == [3] || shape == [1, 3]);
                assert_eq!(static_row.is_ok(), shape == [3] || shape == [1, 3]);
                assert_eq!(static_column.is_ok(), shape == [3] || shape == [3, 1]);
                let result = DVector::<f64>::try_read_with_shape(bytes.as_slice(), (3, 1));
                if shape == [3] || shape == [3, 1] {
                    assert_eq!(result.unwrap().as_slice(), &[1., 2., 3.]);
                } else {
                    assert!(matches!(result, Err(NpyError::InvalidShape { .. })));
                }
            }
        }
    }
}
