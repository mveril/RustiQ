use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Sha256Digest;

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactManifest {
    pub path: String,
    pub size: u64,
    pub encoding: String,
    pub dtype: String,
    pub representation: String,
    pub basis_functions: usize,
    pub shape: Vec<usize>,
    pub sha256: Sha256Digest,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{
        AO_ERI_PATH, COMPACT_ERI_REPRESENTATION, FORMAT_NAME, FORMAT_VERSION,
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
                encoding: "npy".to_string(),
                dtype: "<f8".to_string(),
                representation: COMPACT_ERI_REPRESENTATION.to_string(),
                basis_functions: 2,
                shape: vec![6],
                digest: Sha256Digest::from([0x22; 32]),
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
                "      \"encoding\": \"npy\",\n",
                "      \"dtype\": \"<f8\",\n",
                "      \"representation\": \"rustiq-compact-eri-v1\",\n",
                "      \"basis_functions\": 2,\n",
                "      \"shape\": [\n",
                "        6\n",
                "      ],\n",
                "      \"digest\": \"sha256:2222222222222222222222222222222222222222222222222222222222222222\"\n",
                "    }\n",
                "  }\n",
                "}"
            )
        );
        assert_eq!(serde_json::from_str::<Manifest>(&json).unwrap(), manifest);
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
