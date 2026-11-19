//! NPY artifact storage using existing conversions; spin pairs are directories.
use nalgebra::{
    storage::{IsContiguous, Storage as MatrixStorage},
    DMatrix, Dim, Matrix,
};
use npyz::WriterBuilder;
use relative_path::RelativePath;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::npy::NpyConvert;
use super::{ArtifactError, Sha256Digest, Storage};
use crate::calculation::Spin;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PayloadRef {
    path: String,
    pub(crate) size: u64,
    pub(crate) digest: Sha256Digest,
}

pub(crate) trait Payload: Sized {
    type Reference: Serialize + DeserializeOwned;
    type Shape: Copy;
    fn load(
        storage: &Storage,
        stem: &str,
        reference: Self::Reference,
        shape: Self::Shape,
    ) -> Result<Self, ArtifactError>;
}

/// Writes borrowed payload views without requiring an owned matrix or vector buffer.
pub(crate) trait PayloadWrite {
    type Reference;
    fn save_payload(
        &self,
        storage: &mut Storage,
        stem: &str,
    ) -> Result<Self::Reference, ArtifactError>;
}

pub(crate) struct MatrixSlice<'a> {
    pub(crate) values: &'a [f64],
    pub(crate) shape: (usize, usize),
    pub(crate) order: npyz::Order,
}

impl<'a, R: Dim, C: Dim, S> From<&'a Matrix<f64, R, C, S>> for MatrixSlice<'a>
where
    S: MatrixStorage<f64, R, C> + IsContiguous,
{
    fn from(matrix: &'a Matrix<f64, R, C, S>) -> Self {
        Self {
            values: matrix.as_slice(),
            shape: matrix.shape(),
            order: npyz::Order::Fortran,
        }
    }
}

impl PayloadWrite for MatrixSlice<'_> {
    type Reference = PayloadRef;
    fn save_payload(
        &self,
        storage: &mut Storage,
        stem: &str,
    ) -> Result<Self::Reference, ArtifactError> {
        PayloadRef::save_matrix_slice(self.values, self.shape, self.order, storage, stem)
    }
}

impl<T: std::ops::Deref<Target = [f64]>> PayloadWrite for T {
    type Reference = PayloadRef;
    fn save_payload(
        &self,
        storage: &mut Storage,
        stem: &str,
    ) -> Result<Self::Reference, ArtifactError> {
        PayloadRef::save_vector_slice(self, storage, stem)
    }
}

impl<T: PayloadWrite> PayloadWrite for Spin<T> {
    type Reference = Spin<T::Reference>;
    fn save_payload(
        &self,
        storage: &mut Storage,
        stem: &str,
    ) -> Result<Self::Reference, ArtifactError> {
        self.as_ref()
            .zip_map(Spin::new("alpha", "beta"), |value, channel| {
                (value, channel)
            })
            .try_map(|(value, channel)| value.save_payload(storage, &format!("{stem}/{channel}")))
    }
}

fn checked(
    storage: &Storage,
    stem: &str,
    extension: &str,
    reference: &PayloadRef,
) -> Result<(), ArtifactError> {
    if reference.path != format!("{stem}.{extension}") {
        return Err(ArtifactError::InvalidPath(reference.path.clone()));
    }
    let path = RelativePath::new(&reference.path);
    if storage.artifact_size(path)? != reference.size {
        return Err(ArtifactError::IntegrityMismatch(reference.path.clone()));
    }
    let metadata = storage.artifact_metadata(path)?;
    if metadata.digest != reference.digest || metadata.size != reference.size {
        return Err(ArtifactError::IntegrityMismatch(reference.path.clone()));
    }
    Ok(())
}

fn with_npy<T>(
    storage: &Storage,
    stem: &str,
    reference: &PayloadRef,
    read: impl FnOnce(&mut dyn std::io::Read) -> Result<T, ArtifactError>,
) -> Result<T, ArtifactError> {
    checked(storage, stem, "npy", reference)?;
    storage.with_artifact(RelativePath::new(&reference.path), read)
}

impl<T: NpyConvert> Payload for T {
    type Reference = PayloadRef;
    type Shape = T::Shape;
    fn load(
        storage: &Storage,
        stem: &str,
        reference: PayloadRef,
        shape: Self::Shape,
    ) -> Result<Self, ArtifactError> {
        with_npy(storage, stem, &reference, |reader| {
            Ok(T::try_read_with_shape(reader, shape)?)
        })
    }
}

/// Reads the same NPY payload as `T`, verifying finite values without storing them.
pub(crate) struct CheckedNpy<T>(std::marker::PhantomData<T>);

impl<T: NpyConvert> Payload for CheckedNpy<T> {
    type Reference = PayloadRef;
    type Shape = T::Shape;
    fn load(
        storage: &Storage,
        stem: &str,
        reference: PayloadRef,
        shape: Self::Shape,
    ) -> Result<Self, ArtifactError> {
        use std::io::Read;

        use super::npy::{map_npy_value_read_error, read_bounded_header};
        use super::NpyError;

        with_npy(storage, stem, &reference, |reader| {
            let mut reader = std::io::BufReader::new(reader);
            let header = read_bounded_header(&mut reader)?;
            let npy = npyz::NpyFile::new(std::io::Cursor::new(header).chain(reader))
                .map_err(NpyError::Read)?;
            T::check_shape(&npy, shape)?;
            let values = npy
                .data::<f64>()
                .map_err(|error| NpyError::InvalidDtype(error.to_string()))?;
            for value in values {
                if !value.map_err(map_npy_value_read_error)?.is_finite() {
                    return Err(ArtifactError::InvalidMetadata(
                        "NPY payload contains a nonfinite value".into(),
                    ));
                }
            }
            Ok(Self(std::marker::PhantomData))
        })
    }
}

impl<T: Payload> Payload for Spin<T> {
    type Reference = Spin<T::Reference>;
    type Shape = T::Shape;
    fn load(
        storage: &Storage,
        stem: &str,
        reference: Self::Reference,
        shape: Self::Shape,
    ) -> Result<Self, ArtifactError> {
        reference
            .zip_map(Spin::new("alpha", "beta"), |value, channel| {
                (value, channel)
            })
            .try_map(|(value, channel)| {
                T::load(storage, &format!("{stem}/{channel}"), value, shape)
            })
    }
}

impl PayloadRef {
    pub(super) fn save_matrix_slice(
        values: &[f64],
        shape: (usize, usize),
        order: npyz::Order,
        storage: &mut Storage,
        stem: &str,
    ) -> Result<Self, ArtifactError> {
        if shape.0.checked_mul(shape.1) != Some(values.len()) {
            return Err(ArtifactError::InvalidMetadata(
                "matrix payload length mismatch".into(),
            ));
        }
        let matrix_shape = DMatrix::<f64>::npy_shape(shape)?;
        Self::save_npy(values, &matrix_shape, order, storage, stem)
    }

    pub(super) fn save_vector_slice(
        values: &[f64],
        storage: &mut Storage,
        stem: &str,
    ) -> Result<Self, ArtifactError> {
        let shape = [u64::try_from(values.len()).map_err(|_| super::NpyError::DimensionOverflow)?];
        Self::save_npy(values, &shape, npyz::Order::C, storage, stem)
    }

    fn save_npy(
        values: &[f64],
        shape: &[u64],
        order: npyz::Order,
        storage: &mut Storage,
        stem: &str,
    ) -> Result<Self, ArtifactError> {
        let path = format!("{stem}.npy");
        let metadata =
            storage.write_artifact::<ArtifactError, _>(RelativePath::new(&path), |writer| {
                write_slice_npy(values, shape, order, writer)?;
                Ok(())
            })?;
        Ok(Self {
            path,
            size: metadata.size,
            digest: metadata.digest,
        })
    }

    pub(super) fn copy_to(
        &self,
        source: &Storage,
        destination: &mut Storage,
        stem: &str,
    ) -> Result<(), ArtifactError> {
        checked(source, stem, "npy", self)?;
        let path = RelativePath::new(&self.path);
        let metadata = source.with_artifact::<_, ArtifactError, _>(path, |input| {
            destination.write_artifact::<ArtifactError, _>(path, |output| {
                std::io::copy(input, output).map_err(super::StorageError::from)?;
                Ok(())
            })
        })?;
        if metadata.size != self.size || metadata.digest != self.digest {
            return Err(ArtifactError::IntegrityMismatch(self.path.clone()));
        }
        Ok(())
    }
}

// Shared stream encoder for the explicit 1D and matrix payload writers.
fn write_slice_npy(
    values: &[f64],
    shape: &[u64],
    order: npyz::Order,
    writer: impl std::io::Write,
) -> Result<(), super::NpyError> {
    let options = npyz::WriteOptions::new();
    let options = options.default_dtype();
    let options = options.order(order);
    let mut writer = options
        .shape(shape)
        .writer(writer)
        .begin_nd()
        .map_err(super::NpyError::Write)?;
    writer
        .extend(values.iter().copied())
        .map_err(super::NpyError::Write)?;
    writer.finish().map_err(super::NpyError::Write)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::{DMatrix, DVector};
    use proptest::strategy::Strategy;

    #[test]
    fn orbital_check_bounds_the_npy_header() {
        use super::super::{NpyError, StorageError};

        let directory = tempfile::tempdir().unwrap();
        let mut storage = Storage::folder(directory.path());
        let path = "energies.npy";
        let metadata = storage
            .write_artifact::<ArtifactError, _>(RelativePath::new(path), |writer| {
                writer
                    .write_all(b"\x93NUMPY\x02\x00")
                    .map_err(StorageError::from)?;
                writer
                    .write_all(&65_537_u32.to_le_bytes())
                    .map_err(StorageError::from)?;
                Ok(())
            })
            .unwrap();
        let reference = PayloadRef {
            path: path.into(),
            size: metadata.size,
            digest: metadata.digest,
        };
        let result = CheckedNpy::<DVector<f64>>::load(&storage, "energies", reference, (2, 1));
        assert!(matches!(
            result,
            Err(ArtifactError::Npy(NpyError::Read(error)))
                if error.to_string() == "NPY header exceeds 64 KiB"
        ));
    }

    #[test]
    fn check_rejects_nonfinite_orbitals_with_valid_integrity() {
        let directory = tempfile::tempdir().unwrap();
        let mut storage = Storage::folder(directory.path());
        let values = [1.0, f64::NAN];
        let reference = values
            .as_slice()
            .save_payload(&mut storage, "energies")
            .unwrap();
        assert!(matches!(
            CheckedNpy::<DVector<f64>>::load(&storage, "energies", reference, (2, 1)),
            Err(ArtifactError::InvalidMetadata(_))
        ));
    }

    proptest::proptest! {
        #[test]
        fn matrices_interoperate_between_binary_writers_and_readers(
            case in (0usize..7, 0usize..7).prop_flat_map(|(rows, columns)| {
                proptest::collection::vec(-1.0e6_f64..1.0e6, rows * columns)
                    .prop_map(move |values| (rows, columns, values))
            }),
        ) {
            use std::io::Cursor;
            let (rows, columns, values) = case;
            let matrix = DMatrix::from_row_slice(rows, columns, &values);
            let view: MatrixSlice<'_> = (&matrix).into();
            proptest::prop_assert_eq!(view.values.as_ptr(), matrix.as_slice().as_ptr());
            let shape = [u64::try_from(rows).unwrap(), u64::try_from(columns).unwrap()];
            let mut native = Cursor::new(Vec::new());
            matrix.write_npy(&mut native).unwrap();
            for order in [npyz::Order::C, npyz::Order::Fortran] {
                let ordered = if order == npyz::Order::C { values.as_slice() } else { view.values };
                let mut sliced = Cursor::new(Vec::new());
                write_slice_npy(ordered, &shape, order, &mut sliced).unwrap();
                for (bytes, expected_order) in [(sliced.get_ref(), order), (native.get_ref(), npyz::Order::Fortran)] {
                    let header = npyz::NpyFile::new(Cursor::new(bytes)).unwrap();
                    proptest::prop_assert_eq!(header.shape(), &shape);
                    proptest::prop_assert_eq!(header.order(), expected_order);
                    let restored = DMatrix::<f64>::try_read_with_shape(Cursor::new(bytes), (rows, columns)).unwrap();
                    proptest::prop_assert_eq!(&restored, &matrix);
                    proptest::prop_assert!(DMatrix::<f64>::try_read_with_shape(Cursor::new(bytes), (rows + 1, columns)).is_err());
                    if rows != columns {
                        proptest::prop_assert!(DMatrix::<f64>::try_read_with_shape(Cursor::new(bytes), (columns, rows)).is_err());
                    }
                }
            }
        }

        #[test]
        fn slices_interoperate_with_dynamic_nalgebra_vectors(
            values in proptest::collection::vec(-1.0e6_f64..1.0e6, 0..33),
        ) {
            use std::io::Cursor;
            use nalgebra::RowDVector;
            let n = values.len();
            let mut bytes = Cursor::new(Vec::new());
            write_slice_npy(&values, &[u64::try_from(n).unwrap()], npyz::Order::C, &mut bytes).unwrap();
            let column = DVector::<f64>::try_read_with_shape(Cursor::new(bytes.get_ref()), (n, 1)).unwrap();
            let row = RowDVector::<f64>::try_read_with_shape(Cursor::new(bytes.get_ref()), (1, n)).unwrap();
            proptest::prop_assert_eq!(column.as_slice(), values.as_slice());
            proptest::prop_assert_eq!(row.as_slice(), values.as_slice());
            proptest::prop_assert!(matches!(DMatrix::<f64>::try_read_with_shape(Cursor::new(bytes.get_ref()), (n, 1)), Err(super::super::NpyError::InvalidShape { .. })), "1D input must be rejected by DMatrix");
            for (shape, encoded) in [
                ([u64::try_from(n).unwrap(), 1], { let mut bytes = Vec::new(); column.write_npy(&mut bytes).unwrap(); bytes }),
                ([1, u64::try_from(n).unwrap()], { let mut bytes = Vec::new(); row.write_npy(&mut bytes).unwrap(); bytes }),
            ] {
                let npy = npyz::NpyFile::new(Cursor::new(encoded)).unwrap();
                proptest::prop_assert_eq!(npy.shape(), &shape);
                proptest::prop_assert_eq!(npy.into_vec::<f64>().unwrap(), values.as_slice());
            }
        }

        #[test]
        fn slices_interoperate_with_static_nalgebra_vectors(
            values in proptest::array::uniform3(-1.0e6_f64..1.0e6),
        ) {
            use std::io::Cursor;
            use nalgebra::{RowVector3, Vector3};
            let mut bytes = Cursor::new(Vec::new());
            write_slice_npy(&values, &[3], npyz::Order::C, &mut bytes).unwrap();
            let column = Vector3::<f64>::try_read_with_shape(Cursor::new(bytes.get_ref()), (3, 1)).unwrap();
            let row = RowVector3::<f64>::try_read_with_shape(Cursor::new(bytes.get_ref()), (1, 3)).unwrap();
            proptest::prop_assert_eq!(column.as_slice(), &values);
            proptest::prop_assert_eq!(row.as_slice(), &values);
            proptest::prop_assert!(Vector3::<f64>::try_read_with_shape(Cursor::new(bytes.get_ref()), (2, 1)).is_err());
            let mut encoded = Vec::new();
            column.write_npy(&mut encoded).unwrap();
            let npy = npyz::NpyFile::new(Cursor::new(encoded)).unwrap();
            proptest::prop_assert_eq!(npy.shape(), &[3, 1]);
            let restored = npy.into_vec::<f64>().unwrap();
            proptest::prop_assert_eq!(restored.as_slice(), &values);
        }
    }

    #[test]
    fn rust_array_npy_stream_loads_as_static_and_dynamic_nalgebra_vectors() {
        use nalgebra::{RowDVector, RowVector3, Vector3};
        use std::io::Cursor;

        let array = [1.25, -2.5, 7.];
        let mut stream = Cursor::new(Vec::new());
        write_slice_npy(&array, &[3], npyz::Order::C, &mut stream).unwrap();
        let bytes = stream.get_ref();
        let column = Vector3::<f64>::try_read_with_shape(Cursor::new(bytes), (3, 1)).unwrap();
        let row = RowVector3::<f64>::try_read_with_shape(Cursor::new(bytes), (1, 3)).unwrap();
        let dynamic_column =
            DVector::<f64>::try_read_with_shape(Cursor::new(bytes), (3, 1)).unwrap();
        let dynamic_row =
            RowDVector::<f64>::try_read_with_shape(Cursor::new(bytes), (1, 3)).unwrap();
        assert_eq!(column.as_slice(), array);
        assert_eq!(row.as_slice(), array);
        assert_eq!(dynamic_column.as_slice(), array);
        assert_eq!(dynamic_row.as_slice(), array);
        assert_eq!(column.shape(), (3, 1));
        assert_eq!(row.shape(), (1, 3));
        assert_eq!(dynamic_column.shape(), (3, 1));
        assert_eq!(dynamic_row.shape(), (1, 3));
    }

    #[test]
    fn binary_interop_between_existing_rust_array_and_nalgebra_conversions() {
        use nalgebra::{RowDVector, RowVector3, Vector3};
        use std::io::Cursor;

        let array = [1.25, -2.5, 7.];
        let mut flat = Cursor::new(Vec::new());
        write_slice_npy(&array, &[3], npyz::Order::C, &mut flat).unwrap();
        assert_eq!(
            npyz::NpyFile::new(Cursor::new(flat.get_ref()))
                .unwrap()
                .shape(),
            &[3]
        );
        let column =
            Vector3::<f64>::try_read_with_shape(Cursor::new(flat.get_ref()), (3, 1)).unwrap();
        let row =
            RowVector3::<f64>::try_read_with_shape(Cursor::new(flat.get_ref()), (1, 3)).unwrap();
        let column_array: [f64; 3] = column.into();
        let row_array: [f64; 3] = row.into();
        assert_eq!(column_array, array);
        assert_eq!(row_array, array);

        // Existing From<[T; N]> conversions followed by each nalgebra writer.
        for (shape, bytes) in [
            ([3, 1], {
                let mut bytes = Vec::new();
                Vector3::from(array).write_npy(&mut bytes).unwrap();
                bytes
            }),
            ([1, 3], {
                let mut bytes = Vec::new();
                RowVector3::from(array).write_npy(&mut bytes).unwrap();
                bytes
            }),
        ] {
            let npy = npyz::NpyFile::new(Cursor::new(bytes)).unwrap();
            assert_eq!(npy.shape(), shape);
            let decoded: Vec<f64> = npy.into_vec().unwrap();
            let decoded_array: [f64; 3] = decoded.try_into().unwrap();
            assert_eq!(decoded_array, array);
        }

        // Existing dynamic conversions transfer the allocation without copying.
        let values = array.to_vec();
        let pointer = values.as_ptr();
        let column = DVector::from(values);
        assert_eq!(column.as_slice().as_ptr(), pointer);
        let values: Vec<f64> = column.data.into();
        assert_eq!(values.as_ptr(), pointer);
        let row = RowDVector::from(values);
        assert_eq!(row.as_slice().as_ptr(), pointer);
        let values: Vec<f64> = row.data.into();
        assert_eq!(values.as_ptr(), pointer);
        assert_eq!(values, array);
    }

    #[test]
    fn binary_stream_interoperability_between_slice_and_nalgebra_contracts() {
        use nalgebra::{RowDVector, RowVector3, SMatrix, SVector, Vector3};
        use std::io::Cursor;

        // All buffers stay in memory. Non-symmetric values expose ordering errors.
        for (rows, columns) in [(1, 1), (1, 3), (3, 1), (2, 3)] {
            let values = [1., -2., 3., 4., -5., 6.];
            let values = &values[..rows * columns];
            let expected = DMatrix::from_row_slice(rows, columns, values);
            let view: MatrixSlice<'_> = (&expected).into();
            let mut native = Cursor::new(Vec::new());
            expected.write_npy(&mut native).unwrap();
            for order in [npyz::Order::C, npyz::Order::Fortran] {
                let mut stream = Cursor::new(Vec::new());
                let shape = [
                    u64::try_from(rows).unwrap(),
                    u64::try_from(columns).unwrap(),
                ];
                let ordered = if order == npyz::Order::C {
                    values
                } else {
                    view.values
                };
                write_slice_npy(ordered, &shape, order, &mut stream).unwrap();
                let header = npyz::NpyFile::new(stream.get_ref().as_slice()).unwrap();
                assert_eq!(header.shape(), shape);
                assert_eq!(header.order(), order);
                for bytes in [stream.get_ref(), native.get_ref()] {
                    assert_eq!(
                        DMatrix::<f64>::try_read_with_shape(Cursor::new(bytes), (rows, columns))
                            .unwrap(),
                        expected
                    );
                    if (rows, columns) == (2, 3) {
                        assert_eq!(
                            SMatrix::<f64, 2, 3>::try_read_with_shape(Cursor::new(bytes), (2, 3))
                                .unwrap()
                                .as_slice(),
                            expected.as_slice()
                        );
                    }
                }
            }
        }
        for values in [&[7.][..], &[1., -2., 3.][..]] {
            let n = values.len();
            let mut flat = Cursor::new(Vec::new());
            write_slice_npy(
                values,
                &[u64::try_from(n).unwrap()],
                npyz::Order::C,
                &mut flat,
            )
            .unwrap();
            let bytes = flat.get_ref();
            assert_eq!(
                DVector::<f64>::try_read_with_shape(Cursor::new(bytes), (n, 1))
                    .unwrap()
                    .as_slice(),
                values
            );
            assert_eq!(
                RowDVector::<f64>::try_read_with_shape(Cursor::new(bytes), (1, n))
                    .unwrap()
                    .as_slice(),
                values
            );
            assert!(DMatrix::<f64>::try_read_with_shape(Cursor::new(bytes), (n, 1)).is_err());
            if n == 1 {
                assert_eq!(
                    SVector::<f64, 1>::try_read_with_shape(Cursor::new(bytes), (1, 1)).unwrap()[0],
                    values[0]
                );
                assert_eq!(
                    SMatrix::<f64, 1, 1>::try_read_with_shape(Cursor::new(bytes), (1, 1)).unwrap()
                        [(0, 0)],
                    values[0]
                );
            } else {
                assert_eq!(
                    Vector3::<f64>::try_read_with_shape(Cursor::new(bytes), (3, 1))
                        .unwrap()
                        .as_slice(),
                    values
                );
                assert_eq!(
                    RowVector3::<f64>::try_read_with_shape(Cursor::new(bytes), (1, 3))
                        .unwrap()
                        .as_slice(),
                    values
                );
            }
        }
    }

    #[test]
    fn vector_payload_accepts_slice_owners_without_copying() {
        fn roundtrip<T: std::ops::Deref<Target = [f64]>>(values: T) {
            let directory = tempfile::tempdir().unwrap();
            let mut storage = Storage::folder(directory.path());
            let pointer = values.as_ptr();
            let reference = values.save_payload(&mut storage, "values").unwrap();
            assert_eq!(values.as_ptr(), pointer);
            let restored =
                DVector::<f64>::load(&storage, "values", reference, (values.len(), 1)).unwrap();
            assert_eq!(restored.as_slice(), &*values);
        }
        roundtrip(vec![1., 2., 3.]);
        roundtrip(vec![1., 2., 3.].into_boxed_slice());
        roundtrip(&[1., 2., 3.][..]);
        roundtrip(std::sync::Arc::<[f64]>::from([1., 2., 3.]));
    }

    #[test]
    fn spin_npy_payloads_roundtrip_and_check_integrity() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = Storage::folder(dir.path());
        let source = Spin::new(
            DVector::from_vec(vec![1.0, 2.0]),
            DVector::from_vec(vec![3.0, 4.0]),
        );
        let reference = Spin::new(source.alpha().as_slice(), source.beta().as_slice())
            .save_payload(&mut storage, "energies")
            .unwrap();
        assert!(dir.path().join("energies/alpha.npy").is_file());
        let restored =
            Spin::<DVector<f64>>::load(&storage, "energies", reference.clone(), (2, 1)).unwrap();
        assert_eq!(source.alpha(), restored.alpha());
        assert_eq!(source.beta(), restored.beta());
        std::fs::write(dir.path().join("energies/beta.npy"), b"corrupt").unwrap();
        assert!(Spin::<DVector<f64>>::load(&storage, "energies", reference, (2, 1)).is_err());

        let json = serde_json::to_value(Spin::new(1, 2)).unwrap();
        assert_eq!(json, serde_json::json!({"alpha": 1, "beta": 2}));
    }

    #[test]
    fn spin_matrix_payloads_roundtrip_through_alpha_beta_paths() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = Storage::folder(dir.path());
        let source = Spin::new(
            DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 4.0]),
            DMatrix::from_row_slice(2, 2, &[5.0, 6.0, 7.0, 8.0]),
        );
        let reference = Spin::new(
            MatrixSlice {
                values: source.alpha().as_slice(),
                shape: (2, 2),
                order: npyz::Order::Fortran,
            },
            MatrixSlice {
                values: source.beta().as_slice(),
                shape: (2, 2),
                order: npyz::Order::Fortran,
            },
        )
        .save_payload(&mut storage, "coefficients")
        .unwrap();
        let alpha_bytes = std::fs::read(dir.path().join("coefficients/alpha.npy")).unwrap();
        assert_eq!(
            npyz::NpyFile::new(alpha_bytes.as_slice()).unwrap().shape(),
            &[2, 2]
        );
        assert!(dir.path().join("coefficients/alpha.npy").is_file());
        assert!(dir.path().join("coefficients/beta.npy").is_file());
        let restored =
            Spin::<DMatrix<f64>>::load(&storage, "coefficients", reference, (2, 2)).unwrap();
        assert_eq!(source.alpha(), restored.alpha());
        assert_eq!(source.beta(), restored.beta());
    }

    #[test]
    fn matrix_views_preserve_c_and_fortran_order() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = Storage::folder(dir.path());
        let expected = DMatrix::from_row_slice(2, 3, &[1., 2., 3., 4., 5., 6.]);
        let borrowed: MatrixSlice<'_> = (&expected).into();
        assert_eq!(borrowed.values.as_ptr(), expected.as_slice().as_ptr());
        for (stem, view) in [
            ("fortran", borrowed),
            (
                "c",
                MatrixSlice {
                    values: &[1., 2., 3., 4., 5., 6.],
                    shape: (2, 3),
                    order: npyz::Order::C,
                },
            ),
        ] {
            let reference = view.save_payload(&mut storage, stem).unwrap();
            let bytes = std::fs::read(dir.path().join(format!("{stem}.npy"))).unwrap();
            assert_eq!(
                npyz::NpyFile::new(bytes.as_slice()).unwrap().order(),
                view.order
            );
            let restored = DMatrix::<f64>::load(&storage, stem, reference, (2, 3)).unwrap();
            assert_eq!(restored, expected);
            let pointer = restored.as_slice().as_ptr();
            let values: Vec<f64> = restored.data.into();
            assert_eq!(values.as_ptr(), pointer);
        }
    }

    #[test]
    fn explicit_vector_slice_writer_emits_one_dimensional_shape() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = Storage::folder(dir.path());
        let reference = [7.0_f64]
            .as_slice()
            .save_payload(&mut storage, "energy")
            .unwrap();
        let bytes = std::fs::read(dir.path().join("energy.npy")).unwrap();
        assert_eq!(npyz::NpyFile::new(bytes.as_slice()).unwrap().shape(), &[1]);
        assert_eq!(
            DVector::<f64>::load(&storage, "energy", reference, (1, 1))
                .unwrap()
                .as_slice(),
            &[7.]
        );
    }

    #[test]
    fn matrix_shape_is_checked_before_loading_and_paths_are_fixed() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = Storage::folder(dir.path());
        let source = DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let reference = MatrixSlice {
            values: source.as_slice(),
            shape: (2, 3),
            order: npyz::Order::Fortran,
        }
        .save_payload(&mut storage, "coefficients")
        .unwrap();
        assert!(DMatrix::<f64>::load(&storage, "coefficients", reference.clone(), (3, 2)).is_err());
        assert!(DMatrix::<f64>::load(&storage, "elsewhere", reference, (2, 3)).is_err());
    }
}
