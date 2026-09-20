use crate::{
    basis::Basis,
    config::validated::PositiveFiniteF64,
    molecules::geometry::Geometry,
};

use super::{sha256, Sha256Digest, COMPACT_ERI_REPRESENTATION, SCIENTIFIC_IDENTITY_VERSION};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScientificIdentity {
    pub(crate) version: u32,
    pub(crate) digest: Sha256Digest,
}

/// Computes the AO ERI identity from effective, ordered scientific inputs.
///
/// The geometry must already be expressed in Bohr. The encoding uses
/// length-prefixed collections, big-endian integers and IEEE-754 binary64 bits
/// with signed zero normalized. It intentionally does not use Serde or Rust
/// layouts.
pub(crate) fn ao_eri_identity(
    geometry: &Geometry,
    basis: &Basis,
    schwarz_threshold: Option<PositiveFiniteF64>,
) -> ScientificIdentity {
    let mut bytes = CanonicalBytes::default();
    bytes.text(b"scientific-identity-v1");
    bytes.text(COMPACT_ERI_REPRESENTATION.as_bytes());

    bytes.len(geometry.atoms.len());
    for atom in &geometry.atoms {
        bytes.u32(atom.element.atomic_number);
        bytes.f64(atom.position.x);
        bytes.f64(atom.position.y);
        bytes.f64(atom.position.z);
    }

    // Encode the effective AO data consumed by the ERI engine, in AO order.
    bytes.len(basis.normalized_components.len());
    for (shell_id, components) in basis.shell_ids.iter().zip(&basis.normalized_components) {
        let origin = basis.shells[*shell_id].origin;
        for coordinate in origin.coords.iter() {
            bytes.f64(*coordinate);
        }

        bytes.len(components.len());
        for component in components {
            for axis in component.angular_momentum.iter() {
                bytes.u8(*axis);
            }
            bytes.len(component.primitives.len());
            for primitive in &component.primitives {
                bytes.f64(primitive.exponent);
                bytes.f64(primitive.coefficient);
            }
        }
    }

    match schwarz_threshold {
        None => bytes.u8(0),
        Some(threshold) => {
            bytes.u8(1);
            bytes.f64(threshold.into_inner());
        }
    }

    ScientificIdentity {
        version: SCIENTIFIC_IDENTITY_VERSION,
        digest: sha256(&bytes.0),
    }
}

#[derive(Default)]
struct CanonicalBytes(Vec<u8>);

impl CanonicalBytes {
    fn u8(&mut self, value: u8) {
        self.0.push(value);
    }

    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn len(&mut self, value: usize) {
        self.0.extend_from_slice(&(value as u64).to_be_bytes());
    }

    fn f64(&mut self, value: f64) {
        let canonical = if value == 0.0 { 0.0 } else { value };
        self.0.extend_from_slice(&canonical.to_bits().to_be_bytes());
    }

    fn text(&mut self, value: &[u8]) {
        self.len(value.len());
        self.0.extend_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::DEFAULT_ERI_SCHWARZ_THRESHOLD,
        test_utils::load_sto3g_basis,
    };

    fn input() -> (Geometry, Basis) {
        let geometry = Geometry::from_source(
            "identity-test.xyz",
            "2\nH2 in Bohr\nH 0.0 0.0 0.0\nH 1.4 0.0 0.0\n",
        )
        .unwrap();
        let basis = load_sto3g_basis(&geometry);
        (geometry, basis)
    }

    fn threshold(value: f64) -> Option<PositiveFiniteF64> {
        Some(PositiveFiniteF64::try_new(value).unwrap())
    }

    fn default_threshold() -> Option<PositiveFiniteF64> {
        threshold(DEFAULT_ERI_SCHWARZ_THRESHOLD)
    }

    #[test]
    fn identity_is_deterministic() {
        let (geometry, basis) = input();
        let identity = ao_eri_identity(&geometry, &basis, default_threshold());
        assert_eq!(
            identity,
            ao_eri_identity(&geometry, &basis, default_threshold())
        );
    }

    #[test]
    fn identity_changes_with_geometry() {
        let (mut geometry, basis) = input();
        let original = ao_eri_identity(&geometry, &basis, default_threshold());
        geometry.atoms[1].position.x += 1e-9;
        assert_ne!(
            original,
            ao_eri_identity(&geometry, &basis, default_threshold())
        );
    }

    #[test]
    fn identity_changes_with_effective_basis() {
        let (geometry, mut basis) = input();
        let original = ao_eri_identity(&geometry, &basis, default_threshold());
        basis.normalized_components[0][0].primitives[0].coefficient += 1e-9;
        assert_ne!(
            original,
            ao_eri_identity(&geometry, &basis, default_threshold())
        );
    }

    #[test]
    fn identity_changes_with_screening_settings() {
        let (geometry, basis) = input();
        let default = ao_eri_identity(&geometry, &basis, default_threshold());
        assert_ne!(default, ao_eri_identity(&geometry, &basis, threshold(1e-10)));
        assert_ne!(default, ao_eri_identity(&geometry, &basis, None));
    }

    #[test]
    fn identity_canonicalizes_signed_zero() {
        let (mut geometry, basis) = input();
        geometry.atoms[0].position.x = -0.0;
        let negative_zero = ao_eri_identity(&geometry, &basis, default_threshold());
        geometry.atoms[0].position.x = 0.0;
        assert_eq!(
            negative_zero,
            ao_eri_identity(&geometry, &basis, default_threshold())
        );
    }
}
