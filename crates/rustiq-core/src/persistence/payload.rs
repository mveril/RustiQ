//! NPY artifact storage using existing conversions; spin pairs are directories.
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
    fn save(&self, storage: &mut Storage, stem: &str) -> Result<Self::Reference, ArtifactError>;
    fn load(
        storage: &mut Storage,
        stem: &str,
        reference: Self::Reference,
        shape: Self::Shape,
    ) -> Result<Self, ArtifactError>;
}

fn checked(
    storage: &mut Storage,
    stem: &str,
    extension: &str,
    reference: &PayloadRef,
) -> Result<(), ArtifactError> {
    if reference.path != format!("{stem}.{extension}") {
        return Err(ArtifactError::InvalidPath(reference.path.clone()));
    }
    let path = RelativePath::new(&reference.path);
    if reference.size > 256 * 1024 * 1024 || storage.artifact_size(path)? != reference.size {
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
    fn save(&self, storage: &mut Storage, stem: &str) -> Result<PayloadRef, ArtifactError> {
        let path = format!("{stem}.npy");
        let metadata = storage
            .write_artifact::<ArtifactError, _>(RelativePath::new(&path), |writer| {
                Ok(self.write_npy(writer)?)
            })?;
        Ok(PayloadRef {
            path,
            size: metadata.size,
            digest: metadata.digest,
        })
    }
    fn load(
        storage: &mut Storage,
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
    fn save(&self, storage: &mut Storage, stem: &str) -> Result<Self::Reference, ArtifactError> {
        Ok(Spin::new(
            self.alpha().save(storage, &format!("{stem}/alpha"))?,
            self.beta().save(storage, &format!("{stem}/beta"))?,
        ))
    }
    fn load(
        storage: &mut Storage,
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

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::{DMatrix, DVector};

    #[test]
    fn spin_npy_payloads_roundtrip_and_check_integrity() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = Storage::folder(dir.path());
        let source = Spin::new(
            DVector::from_vec(vec![1.0, 2.0]),
            DVector::from_vec(vec![3.0, 4.0]),
        );
        let reference = source.save(&mut storage, "energies").unwrap();
        assert!(dir.path().join("energies/alpha.npy").is_file());
        let restored =
            Spin::<DVector<f64>>::load(&mut storage, "energies", reference.clone(), 2).unwrap();
        assert_eq!(source.alpha(), restored.alpha());
        assert_eq!(source.beta(), restored.beta());
        std::fs::write(dir.path().join("energies/beta.npy"), b"corrupt").unwrap();
        assert!(Spin::<DVector<f64>>::load(&mut storage, "energies", reference, 2).is_err());

        let json = serde_json::to_value(Spin::new(1, 2)).unwrap();
        assert_eq!(json, serde_json::json!({"alpha": 1, "beta": 2}));
    }

    #[test]
    fn matrix_shape_is_checked_before_loading_and_paths_are_fixed() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = Storage::folder(dir.path());
        let source = DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let reference = source.save(&mut storage, "coefficients").unwrap();
        assert!(
            DMatrix::<f64>::load(&mut storage, "coefficients", reference.clone(), (3, 2)).is_err()
        );
        assert!(DMatrix::<f64>::load(&mut storage, "elsewhere", reference, (2, 3)).is_err());
    }
}
