use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::{fmt, io::{self, Read}, str::FromStr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sha256Digest([u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sha256DigestParseError;

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

impl fmt::Display for Sha256DigestParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("expected sha256: followed by 64 lowercase hexadecimal digits")
    }
}

impl std::error::Error for Sha256DigestParseError {}

impl FromStr for Sha256Digest {
    type Err = Sha256DigestParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let hex = value
            .strip_prefix("sha256:")
            .ok_or(Sha256DigestParseError)?;
        if hex.len() != 64
            || !hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(Sha256DigestParseError);
        }

        let mut bytes = [0_u8; 32];
        for (index, slot) in bytes.iter_mut().enumerate() {
            let offset = index * 2;
            *slot = u8::from_str_radix(&hex[offset..offset + 2], 16)
                .map_err(|_| Sha256DigestParseError)?;
        }
        Ok(Self(bytes))
    }
}

impl Serialize for Sha256Digest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

pub fn sha256(bytes: &[u8]) -> Sha256Digest {
    Sha256::digest(bytes).into()
}

/// Computes a SHA-256 digest without materializing the complete input.
pub fn sha256_reader(mut reader: impl Read) -> Result<Sha256Digest, io::Error> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(hasher.finalize().into());
        }
        hasher.update(&buffer[..read]);
    }
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

    #[test]
    fn digest_string_round_trip_is_strict() {
        let digest = sha256(b"round trip");
        assert_eq!(digest.to_string().parse::<Sha256Digest>().unwrap(), digest);

        assert!("ba7816bf".parse::<Sha256Digest>().is_err());
        assert!(format!("sha256:{}", "A".repeat(64))
            .parse::<Sha256Digest>()
            .is_err());
        assert!(format!("sha256:{}", "g".repeat(64))
            .parse::<Sha256Digest>()
            .is_err());
    }

    #[test]
    fn digest_serde_round_trip_uses_prefixed_string() {
        let digest = Sha256Digest::from([0xab; 32]);
        let json = serde_json::to_string(&digest).unwrap();
        assert_eq!(json, format!("\"sha256:{}\"", "ab".repeat(32)));
        assert_eq!(serde_json::from_str::<Sha256Digest>(&json).unwrap(), digest);
    }
}
