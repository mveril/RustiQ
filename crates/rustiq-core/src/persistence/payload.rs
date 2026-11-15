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
        Ok(Spin::new(
            self.alpha()
                .save_payload(storage, &format!("{stem}/alpha"))?,
            self.beta().save_payload(storage, &format!("{stem}/beta"))?,
        ))
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

impl<T: NpyConvert> Payload for T {
    type Reference = PayloadRef;
    type Shape = T::Shape;
    fn load(
        storage: &Storage,
        stem: &str,
        reference: PayloadRef,
        shape: Self::Shape,
    ) -> Result<Self, ArtifactError> {
        checked(storage, stem, "npy", &reference)?;
        storage.with_artifact::<_, ArtifactError, _>(RelativePath::new(&reference.path), |reader| {
            Ok(T::try_read_with_shape(reader, shape)?)
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
        let (alpha, beta) = reference.into_parts();
        Ok(Spin::new(
            T::load(storage, &format!("{stem}/alpha"), alpha, shape)?,
            T::load(storage, &format!("{stem}/beta"), beta, shape)?,
        ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::{DMatrix, DVector};

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
