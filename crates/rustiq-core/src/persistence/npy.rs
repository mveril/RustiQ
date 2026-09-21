use std::io::{Read, Seek, Write};

use npyz::{DType, NpyFile, TypeChar, WriterBuilder};

use crate::eri::CompactEri;

use super::PersistenceError;

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
    let npy = NpyFile::new(reader).map_err(PersistenceError::NpyRead)?;
    if npy.shape().len() != 1 {
        return Err(PersistenceError::InvalidShape(npy.shape().to_vec()));
    }
    let expected = CompactEri::storage_len(basis_functions);
    let actual = usize::try_from(npy.shape()[0]).unwrap_or(usize::MAX);
    if actual != expected {
        return Err(PersistenceError::InvalidValueCount {
            basis_functions,
            expected,
            actual,
        });
    }
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

pub(crate) fn validate_compact_eri_header(
    reader: impl Read + Seek,
    basis_functions: usize,
    file_size: u64,
) -> bool {
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
    let expected = CompactEri::storage_len(basis_functions) as u64;
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
