use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::{Sha256Digest, COMPACT_ERI_REPRESENTATION};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub format_version: u32,
    pub kind: String,
    pub producer: Producer,
    pub scientific_identity: ScientificIdentityManifest,
    pub artifacts: BTreeMap<String, ArtifactManifest>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Producer {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScientificIdentityManifest {
    pub version: u32,
    pub digest: Sha256Digest,
}

/// Representation-specific metadata for a persisted artifact.
///
/// Known representations are decoded into typed variants. Unknown representations
/// retain their attributes so newer manifests remain inspectable by older readers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum ArtifactAttributes {
    AoEri(AoEriAttributes),
    Unknown(BTreeMap<String, Value>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AoEriAttributes {
    pub basis_functions: usize,
    pub computation_version: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ArtifactManifest {
    pub path: String,
    pub size: u64,
    pub representation: String,
    pub digest: Sha256Digest,
    pub attributes: ArtifactAttributes,
}

#[derive(Deserialize)]
struct RawArtifactManifest {
    path: String,
    size: u64,
    representation: String,
    digest: Sha256Digest,
    #[serde(default)]
    attributes: BTreeMap<String, Value>,
}

impl<'de> Deserialize<'de> for ArtifactManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawArtifactManifest::deserialize(deserializer)?;
        let attributes =
            decode_attributes(&raw.representation, raw.attributes).map_err(serde::de::Error::custom)?;

        Ok(Self {
            path: raw.path,
            size: raw.size,
            representation: raw.representation,
            digest: raw.digest,
            attributes,
        })
    }
}

fn decode_attributes(
    representation: &str,
    raw: BTreeMap<String, Value>,
) -> Result<ArtifactAttributes, serde_json::Error> {
    if representation == COMPACT_ERI_REPRESENTATION {
        let value = Value::Object(raw.into_iter().collect());
        return serde_json::from_value(value).map(ArtifactAttributes::AoEri);
    }

    Ok(ArtifactAttributes::Unknown(raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{
        AO_ERI_COMPUTATION_VERSION, AO_ERI_PATH, FORMAT_NAME, FORMAT_VERSION,
        SCIENTIFIC_IDENTITY_VERSION,
    };

    #[test]
    fn manifest_v1_json_matches_golden_shape() {
        let mut artifacts = BTreeMap::new();
        artifacts.insert(
            "ao_eri".to_string(),
            ArtifactManifest {
                path: AO_ERI_PATH.to_string(),
                size: 176,
                representation: COMPACT_ERI_REPRESENTATION.to_string(),
                digest: Sha256Digest::from([0x22; 32]),
                attributes: ArtifactAttributes::AoEri(AoEriAttributes {
                    basis_functions: 2,
                    computation_version: AO_ERI_COMPUTATION_VERSION,
                }),
            },
        );

        let manifest = Manifest {
            format: FORMAT_NAME.to_string(),
            format_version: FORMAT_VERSION,
            kind: "integral-cache".to_string(),
            producer: Producer {
                name: "RustiQ".to_string(),
                version: "0.1.0".to_string(),
            },
            scientific_identity: ScientificIdentityManifest {
                version: SCIENTIFIC_IDENTITY_VERSION,
                digest: Sha256Digest::from([0x11; 32]),
            },
            artifacts,
        };

        let json = serde_json::to_string_pretty(&manifest).unwrap();
        assert_eq!(
            json,
            concat!(
                "{\n",
                "  \"format\": \"rustiq-persistence\",\n",
                "  \"format_version\": 1,\n",
                "  \"kind\": \"integral-cache\",\n",
                "  \"producer\": {\n",
                "    \"name\": \"RustiQ\",\n",
                "    \"version\": \"0.1.0\"\n",
                "  },\n",
                "  \"scientific_identity\": {\n",
                "    \"version\": 1,\n",
                "    \"digest\": \"sha256:1111111111111111111111111111111111111111111111111111111111111111\"\n",
                "  },\n",
                "  \"artifacts\": {\n",
                "    \"ao_eri\": {\n",
                "      \"path\": \"arrays/integrals/ao-eri.npy\",\n",
                "      \"size\": 176,\n",
                "      \"representation\": \"rustiq-compact-eri-v1\",\n",
                "      \"digest\": \"sha256:2222222222222222222222222222222222222222222222222222222222222222\",\n",
                "      \"attributes\": {\n",
                "        \"basis_functions\": 2,\n",
                "        \"computation_version\": 1\n",
                "      }\n",
                "    }\n",
                "  }\n",
                "}"
            )
        );
        assert_eq!(serde_json::from_str::<Manifest>(&json).unwrap(), manifest);
    }

    #[test]
    fn unknown_artifact_attributes_are_preserved() {
        let json = r#"{
            "format": "rustiq-persistence",
            "format_version": 1,
            "kind": "checkpoint",
            "producer": {"name": "RustiQ", "version": "0.2.0"},
            "scientific_identity": {
                "version": 1,
                "digest": "sha256:1111111111111111111111111111111111111111111111111111111111111111"
            },
            "artifacts": {
                "future": {
                    "path": "arrays/post-hf/future.npy",
                    "size": 42,
                    "representation": "rustiq-future-state-v3",
                    "digest": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
                    "attributes": {
                        "engine_version": 7,
                        "spin": "alpha"
                    }
                }
            }
        }"#;

        let manifest: Manifest = serde_json::from_str(json).unwrap();
        let artifact = manifest.artifacts.get("future").unwrap();
        let ArtifactAttributes::Unknown(attributes) = &artifact.attributes else {
            panic!("unknown representation must retain raw attributes");
        };
        assert_eq!(
            attributes.get("engine_version"),
            Some(&serde_json::json!(7))
        );
        assert_eq!(attributes.get("spin"), Some(&serde_json::json!("alpha")));

        let encoded = serde_json::to_value(&manifest).unwrap();
        assert_eq!(
            encoded["artifacts"]["future"]["attributes"]["engine_version"],
            serde_json::json!(7)
        );
        assert_eq!(
            encoded["artifacts"]["future"]["attributes"]["spin"],
            serde_json::json!("alpha")
        );
    }

    #[test]
    fn malformed_known_attributes_are_rejected() {
        let json = r#"{
            "format": "rustiq-persistence",
            "format_version": 1,
            "kind": "integral-cache",
            "producer": {"name": "RustiQ", "version": "0.1.0"},
            "scientific_identity": {
                "version": 1,
                "digest": "sha256:1111111111111111111111111111111111111111111111111111111111111111"
            },
            "artifacts": {
                "ao_eri": {
                    "path": "arrays/integrals/ao-eri.npy",
                    "size": 176,
                    "representation": "rustiq-compact-eri-v1",
                    "digest": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
                    "attributes": {
                        "basis_functions": "not-an-integer",
                        "computation_version": 1
                    }
                }
            }
        }"#;

        assert!(serde_json::from_str::<Manifest>(json).is_err());
    }

    #[test]
    fn manifest_rejects_invalid_digest_strings() {
        let json = r#"{
            "format": "rustiq-persistence",
            "format_version": 1,
            "kind": "integral-cache",
            "producer": {"name": "RustiQ", "version": "0.1.0"},
            "scientific_identity": {"version": 1, "digest": "banana"},
            "artifacts": {}
        }"#;

        assert!(serde_json::from_str::<Manifest>(json).is_err());
    }
}
