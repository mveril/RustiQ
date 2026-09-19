use sha2::{Digest, Sha256};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    pub fn to_hex(self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

impl From<[u8; 32]> for Sha256Digest {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl From<sha2::digest::Output<Sha256>> for Sha256Digest {
    fn from(output: sha2::digest::Output<Sha256>) -> Self {
        Self(output.into())
    }
}

impl AsRef<[u8]> for Sha256Digest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("sha256:")?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

pub fn sha256(bytes: &[u8]) -> Sha256Digest {
    Sha256::digest(bytes).into()
}

pub fn verify_sha256(bytes: &[u8], expected: Sha256Digest) -> bool {
    sha256(bytes) == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_published_vector() {
        assert_eq!(
            sha256(b"abc").to_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn corrupted_payload_does_not_verify() {
        let expected = sha256(b"scientific payload");
        assert!(verify_sha256(b"scientific payload", expected));
        assert!(!verify_sha256(b"scientific payloae", expected));
    }

    #[test]
    fn digest_conversions_preserve_bytes() {
        let bytes = [42; 32];
        let digest = Sha256Digest::from(bytes);
        assert_eq!(digest.as_ref(), bytes.as_slice());

        let output = Sha256::digest(b"conversion");
        let expected: [u8; 32] = output.into();
        assert_eq!(Sha256Digest::from(output).as_ref(), expected.as_slice());
    }
}
