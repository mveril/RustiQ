use crate::{basis::Basis, molecules::geometry::Geometry};

use super::{sha256, Sha256Digest, COMPACT_ERI_REPRESENTATION, SCIENTIFIC_IDENTITY_VERSION};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScientificIdentity {
    pub version: u32,
    pub digest: Sha256Digest,
}

/// Computes the AO ERI identity from effective, ordered scientific inputs.
///
/// The encoding uses length-prefixed collections, big-endian integers and
/// IEEE-754 binary64 bits with signed zero normalized. It intentionally does
/// not use Serde or Rust layouts.
pub fn ao_eri_identity(
    geometry: &Geometry,
    basis: &Basis,
    schwarz_threshold: f64,
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

    bytes.len(basis.shells.len());
    for shell in &basis.shells {
        for coordinate in shell.origin.coords.iter() {
            bytes.f64(*coordinate);
        }
        bytes.len(shell.alpha.len());
        for exponent in shell.alpha.iter() {
            bytes.f64(*exponent);
        }
        bytes.len(shell.contr.len());
        for contraction in &shell.contr {
            bytes.u8(contraction.l);
            bytes.u8(u8::from(contraction.pure));
            bytes.len(contraction.coeff.len());
            for coefficient in contraction.coeff.iter() {
                bytes.f64(*coefficient);
            }
        }
    }

    // Explicitly encode AO ordering and spherical/cartesian component expansion.
    bytes.len(basis.shell_ids.len());
    for ((shell_id, angular_momentum), components) in basis
        .shell_ids
        .iter()
        .zip(&basis.angular_momenta)
        .zip(&basis.angular_components)
    {
        bytes.len(*shell_id);
        for component in angular_momentum.iter() {
            bytes.u8(*component);
        }
        bytes.len(components.len());
        for (component, coefficient) in components {
            for axis in component.iter() {
                bytes.u8(*axis);
            }
            bytes.f64(*coefficient);
        }
    }
    bytes.f64(schwarz_threshold);

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
    use crate::test_utils::load_sto3g_basis;

    fn input() -> (Geometry, Basis) {
        let geometry = Geometry::from_source(
            "identity-test.xyz",
            "2\nH2 in Bohr\nH 0.0 0.0 0.0\nH 1.4 0.0 0.0\n",
        )
        .unwrap();
        let basis = load_sto3g_basis(&geometry);
        (geometry, basis)
    }

    #[test]
    fn identity_is_deterministic() {
        let (geometry, basis) = input();
        let identity = ao_eri_identity(&geometry, &basis, 1e-12);
        assert_eq!(identity, ao_eri_identity(&geometry, &basis, 1e-12));
        assert_eq!(
            identity.digest.to_hex(),
            "4a3caf7927b541f70b8357959a9a63eddfbd2f87a9577a3fb9ebd09252474ce9"
        );
    }

    #[test]
    fn identity_changes_with_geometry() {
        let (mut geometry, basis) = input();
        let original = ao_eri_identity(&geometry, &basis, 1e-12);
        geometry.atoms[1].position.x += 1e-9;
        assert_ne!(original, ao_eri_identity(&geometry, &basis, 1e-12));
    }

    #[test]
    fn identity_changes_with_resolved_basis() {
        let (geometry, mut basis) = input();
        let original = ao_eri_identity(&geometry, &basis, 1e-12);
        basis.shells[0].alpha[0] += 1e-9;
        assert_ne!(original, ao_eri_identity(&geometry, &basis, 1e-12));
    }

    #[test]
    fn identity_changes_with_screening_threshold() {
        let (geometry, basis) = input();
        assert_ne!(
            ao_eri_identity(&geometry, &basis, 1e-12),
            ao_eri_identity(&geometry, &basis, 1e-10)
        );
    }

    #[test]
    fn identity_canonicalizes_signed_zero() {
        let (mut geometry, basis) = input();
        geometry.atoms[0].position.x = -0.0;
        let negative_zero = ao_eri_identity(&geometry, &basis, 1e-12);
        geometry.atoms[0].position.x = 0.0;
        assert_eq!(negative_zero, ao_eri_identity(&geometry, &basis, 1e-12));
    }
}
