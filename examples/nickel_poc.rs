//! Deserialize fully evaluated Nickel JSON without teaching the CLI Nickel.
use serde::{Deserialize, Serialize};
use std::{io::Read, num::NonZeroU8, path::PathBuf};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Calculation {
    molecule: Molecule,
    basis: Basis,
    method: Method,
    integrals: Integrals,
    cache: Cache,
    output: Output,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Molecule {
    geometry: PathBuf,
    charge: i32,
    multiplicity: NonZeroU8,
    units: Units,
}

#[derive(Debug, Deserialize, Serialize)]
enum Units {
    Angstrom,
    Bohr,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Basis {
    name: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Method {
    hf: Hf,
    mp2: Option<Mp2>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Hf {
    method: HfMethod,
    max_iterations: std::num::NonZeroUsize,
    convergence_threshold: f64,
    guess: Guess,
    diis: Diis,
    orthogonalization: Orthogonalization,
}

#[derive(Debug, Deserialize, Serialize)]
enum HfMethod {
    Auto,
    Rhf,
    Uhf,
}

#[derive(Debug, Deserialize, Serialize)]
// Serde does not support deny_unknown_fields alongside flatten. Nickel checks
// the closed guess records before they cross this boundary.
#[serde(tag = "type")]
enum Guess {
    CoreHamiltonian {
        perturbation: Option<Distribution>,
    },
    OneElectron {
        perturbation: Option<Distribution>,
    },
    Random {
        #[serde(flatten)]
        distribution: Distribution,
    },
    Zero,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "distribution", deny_unknown_fields)]
enum Distribution {
    Normal {
        mean: f64,
        std_dev: f64,
        seed: Option<u64>,
    },
    Uniform {
        min: f64,
        max: f64,
        seed: Option<u64>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Diis {
    enabled: bool,
    max_history: usize,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Orthogonalization {
    linear_dependency_threshold: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Mp2 {
    frozen_orbitals: usize,
    memory_limit: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Integrals {
    schwarz_threshold: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Cache {
    enabled: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Output {
    scf: ScfOutput,
}

#[derive(Debug, Deserialize, Serialize)]
enum ScfOutput {
    Normal,
    Quiet,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CalculationSet {
    Single(Calculation),
    Multiple(Vec<Calculation>),
}

#[derive(Debug, Serialize)]
struct ResolvedInput {
    calculations: Vec<Calculation>,
}

impl From<CalculationSet> for ResolvedInput {
    fn from(value: CalculationSet) -> Self {
        Self {
            calculations: match value {
                CalculationSet::Single(calculation) => vec![calculation],
                CalculationSet::Multiple(calculations) => calculations,
            },
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut json = String::new();
    std::io::stdin().read_to_string(&mut json)?;
    let calculations: CalculationSet = serde_json::from_str(&json)?;
    let resolved: ResolvedInput = calculations.into();
    println!("{}", serde_json::to_string_pretty(&resolved)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::Path, process::Command};

    fn export(path: &Path) -> std::process::Output {
        Command::new(std::env::var_os("NICKEL_BIN").unwrap_or_else(|| "nickel".into()))
            .args(["export", "--format", "json"])
            .arg(path)
            .output()
            .expect("install Nickel or set NICKEL_BIN")
    }

    fn assert_equivalent(left: &serde_json::Value, right: &serde_json::Value) {
        use serde_json::Value;
        match (left, right) {
            (Value::Number(a), Value::Number(b)) if a.is_f64() || b.is_f64() => {
                // Nickel writes integral floats as integers; the DTO writes f64.
                assert_eq!(a.as_f64(), b.as_f64());
            }
            (Value::Object(a), Value::Object(b)) => {
                assert_eq!(a.len(), b.len());
                for (key, value) in a {
                    assert_equivalent(value, b.get(key).expect("same JSON keys"));
                }
            }
            (Value::Array(a), Value::Array(b)) => {
                assert_eq!(a.len(), b.len());
                for (left, right) in a.iter().zip(b) {
                    assert_equivalent(left, right);
                }
            }
            _ => assert_eq!(left, right),
        }
    }

    #[test]
    #[ignore = "requires the Nickel CLI; run explicitly as documented"]
    fn nickel_exports_deserialize_and_invalid_inputs_fail() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/nickel");
        for name in [
            "import-toml.ncl",
            "single.ncl",
            "multiple.ncl",
            "variants.ncl",
        ] {
            let output = export(&root.join(name));
            assert!(
                output.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let dto: CalculationSet = serde_json::from_slice(&output.stdout)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            let roundtrip = serde_json::to_value(&ResolvedInput::from(dto)).unwrap();
            let mut json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            if json.is_object() {
                json = serde_json::json!({ "calculations": [json] });
            } else {
                json = serde_json::json!({ "calculations": json });
            }
            assert_equivalent(&roundtrip, &json);
            if name == "multiple.ncl" {
                assert_eq!(json["calculations"].as_array().unwrap().len(), 3);
            }
            if name == "import-toml.ncl" {
                assert_eq!(
                    json["calculations"][0]["molecule"]["geometry"],
                    "../molecule.xyz"
                );
                assert_eq!(
                    json["calculations"][0]["method"]["hf"]["diis"]["max_history"],
                    6
                );
                assert_eq!(
                    json["calculations"][0]["method"]["hf"]["guess"]["type"],
                    "CoreHamiltonian"
                );
                assert!(json["calculations"][0]["method"]["mp2"].is_null());
            }
        }
        for entry in std::fs::read_dir(root.join("invalid")).unwrap() {
            let path = entry.unwrap().path();
            let output = export(&path);
            assert!(
                !output.status.success(),
                "{} unexpectedly succeeded",
                path.display()
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("contract") || stderr.contains("missing definition"),
                "{stderr}"
            );
        }
    }
}
