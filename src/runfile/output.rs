use super::RunFile;
use serde::Serialize;

#[derive(Debug, Default, Serialize)]
pub struct OutputConfig {
    pub scf: ScfOutput,
}

#[derive(Debug, Default, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
pub enum ScfOutput {
    #[default]
    Normal,
    Quiet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Defaults {
    Include,
    Omit,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(crate) struct RenderError(pub(crate) String);

pub(crate) fn to_string(value: &impl Serialize) -> Result<String, RenderError> {
    let json = serde_json::to_string(value).map_err(|error| RenderError(error.to_string()))?;
    format_floats(&super::nickel::export_toml(&json)?)
}

fn format_floats(toml: &str) -> Result<String, RenderError> {
    let mut document = toml
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| RenderError(error.to_string()))?;
    format_item(document.as_item_mut())?;
    Ok(document.to_string())
}

fn format_item(item: &mut toml_edit::Item) -> Result<(), RenderError> {
    match item {
        toml_edit::Item::Value(value) => format_value(value)?,
        toml_edit::Item::Table(table) => {
            for (_, item) in table.iter_mut() {
                format_item(item)?;
            }
        }
        toml_edit::Item::ArrayOfTables(tables) => {
            for table in tables.iter_mut() {
                for (_, item) in table.iter_mut() {
                    format_item(item)?;
                }
            }
        }
        toml_edit::Item::None => {}
    }
    Ok(())
}

fn format_value(value: &mut toml_edit::Value) -> Result<(), RenderError> {
    match value {
        toml_edit::Value::Float(float) if float.value().is_finite() => {
            // LowerExp without a precision produces a shortest round-trip representation.
            let scientific = format!("{:e}", float.value());
            if scientific.len() < float.display_repr().len() {
                let mut formatted = scientific
                    .parse::<toml_edit::Value>()
                    .map_err(|error| RenderError(error.to_string()))?;
                *formatted.decor_mut() = value.decor().clone();
                *value = formatted;
            }
        }
        toml_edit::Value::Array(array) => {
            for value in array.iter_mut() {
                format_value(value)?;
            }
        }
        toml_edit::Value::InlineTable(table) => {
            for (_, value) in table.iter_mut() {
                format_value(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub struct TomlOutput<'a> {
    value: &'a RunFile,
    defaults: Defaults,
}
impl RunFile {
    pub fn output(&self, defaults: Defaults) -> TomlOutput<'_> {
        TomlOutput {
            value: self,
            defaults,
        }
    }
}
impl TomlOutput<'_> {
    pub(crate) fn render(&self) -> Result<String, RenderError> {
        let mut item =
            serde_json::to_value(self.value).map_err(|error| RenderError(error.to_string()))?;
        if self.defaults == Defaults::Include {
            return to_string(&item);
        }
        // Nickel supplies one resolved default tree for field comparisons.
        let defaults =
            super::nickel::resolve_toml("[basis]\nname = \"rustiq-default-probe\"\n[method.mp2]\n")
                .map_err(|errors| {
                    RenderError(
                        errors
                            .into_iter()
                            .map(|error| error.message)
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )
                })?;
        let defaults =
            serde_json::to_value(defaults).map_err(|error| RenderError(error.to_string()))?;
        let defaults = defaults
            .pointer("/calculations/0")
            .expect("Nickel resolves a RunFile to one calculation");
        let mut paths = Vec::new();
        collect_fields(&item, &mut Vec::new(), &mut paths);
        for path in paths {
            // Preserve explicitly requested calculation sections in canonical output.
            if path == ["method"] || path == ["method", "hf"] || path == ["method", "mp2"] {
                continue;
            }
            // basis.name is required by the Nickel schema and has no default.
            if path == ["basis"] || path == ["basis", "name"] {
                continue;
            }
            let Some(default) = path.iter().try_fold(defaults, |value, key| value.get(key)) else {
                continue;
            };
            let Some(value) = path.iter().try_fold(&item, |value, key| value.get(key)) else {
                continue;
            };
            if matches_default(value, default) {
                remove_field(&mut item, &path);
            }
        }
        to_string(&item)
    }
}

/// Match frontend JSON to the resolved DTO; omitted optional fields correspond to null.
fn matches_default(value: &serde_json::Value, default: &serde_json::Value) -> bool {
    if value == default {
        return true;
    }
    let (Some(value), Some(default)) = (value.as_object(), default.as_object()) else {
        return false;
    };
    value.keys().chain(default.keys()).all(|key| {
        matches_default(
            value.get(key).unwrap_or(&serde_json::Value::Null),
            default.get(key).unwrap_or(&serde_json::Value::Null),
        )
    })
}

fn collect_fields(item: &serde_json::Value, path: &mut Vec<String>, paths: &mut Vec<Vec<String>>) {
    if let Some(table) = item.as_object() {
        for (key, value) in table {
            path.push(key.clone());
            paths.push(path.clone());
            collect_fields(value, path, paths);
            path.pop();
        }
    }
}
fn remove_field(item: &mut serde_json::Value, path: &[String]) -> bool {
    let Some((key, parents)) = path.split_last() else {
        return false;
    };
    parents
        .iter()
        .try_fold(item, |item, key| item.get_mut(key))
        .and_then(serde_json::Value::as_object_mut)
        .is_some_and(|table| table.remove(key).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runfile::parser::parse_runfile;

    #[test]
    fn scientific_notation_preserves_toml_layout_and_non_float_values() {
        let source = r#"# Keep decimal-looking text intact.
text = "0.00000001"
multiline = """
threshold = 0.00000001
"""
iterations = 1000000
zero = 0.0
negative_zero = -0.0
ordinary = 0.01
threshold = 0.00000001 # convergence
values = [0.000000000001, -0.00000001, { threshold = 0.000001 }]

[nested]
threshold = 0.000000000001

[[calculations]]
threshold = 0.00000001
"#;
        let expected = source
            .replace("threshold = 0.00000001 #", "threshold = 1e-8 #")
            .replace(
                "[0.000000000001, -0.00000001, { threshold = 0.000001 }]",
                "[1e-12, -1e-8, { threshold = 1e-6 }]",
            )
            .replace(
                "[nested]\nthreshold = 0.000000000001",
                "[nested]\nthreshold = 1e-12",
            )
            .replace(
                "[[calculations]]\nthreshold = 0.00000001",
                "[[calculations]]\nthreshold = 1e-8",
            );
        assert_eq!(format_floats(source).unwrap(), expected);
    }

    #[test]
    fn scientific_notation_preserves_float_bits() {
        for number in [
            f64::MIN_POSITIVE,
            f64::from_bits(1),
            f64::MAX,
            1e-12,
            -1e-8,
            1.234_567_890_123_456_7e-12,
            1.234_567_890_123_456_7e20,
            0.01,
            0.0,
            -0.0,
        ] {
            let source = format!("number = {}\n", toml_edit::Value::from(number));
            let output = format_floats(&source).unwrap();
            let parsed = output.parse::<toml_edit::DocumentMut>().unwrap();
            assert_eq!(
                parsed["number"].as_float().unwrap().to_bits(),
                number.to_bits()
            );
        }
    }

    #[test]
    fn output_context_controls_defaults_without_changing_the_model() {
        let source = "[molecule]\n[basis]\nname = \"sto-3g\"\n[method.hf]\n[method.mp2]\n";
        let parsed = parse_runfile("test", source).unwrap();
        let full = parsed.runfile.output(Defaults::Include).render().unwrap();
        let compact = parsed.runfile.output(Defaults::Omit).render().unwrap();
        assert!(full.contains("convergence_threshold = 1e-8"));
        assert!(full.contains("schwarz_threshold = 1e-12"));
        for field in [
            "charge =",
            "multiplicity =",
            "units =",
            "method =",
            "max_iterations =",
            "convergence_threshold =",
            "linear_dependency_threshold =",
            "schwarz_threshold =",
            "enabled =",
            "max_history =",
            "type =",
            "frozen_orbitals =",
            "memory_limit =",
            "scf =",
        ] {
            assert!(full.contains(field), "missing {field}");
            assert!(!compact.contains(field), "unexpected {field}");
        }
        assert!(full.contains("[method.hf.guess]"));
        assert!(!compact.contains("[method.hf.guess]"));
        assert!(compact.contains("[method.hf]"));
        assert!(compact.contains("[method.mp2]"));
        let restored = parse_runfile("compact", &compact).unwrap();
        assert_eq!(
            full,
            restored.runfile.output(Defaults::Include).render().unwrap()
        );
        assert_eq!(
            full,
            parsed.runfile.output(Defaults::Include).render().unwrap()
        );
    }

    #[test]
    fn compact_output_preserves_non_default_and_tagged_configuration() {
        let source = r#"
[molecule]
charge = -1
multiplicity = 2
units = "Bohr"

[basis]
name = "cc-pvdz"

[method.hf]
method = "Uhf"
max_iterations = 42

[method.hf.diis]
enabled = true
max_history = 8

[method.hf.guess]
type = "OneElectron"

[method.hf.guess.perturbation]
distribution = "Normal"
mean = 0.0
std_dev = 0.01

[method.mp2]
frozen_orbitals = 1
"#;
        let parsed = parse_runfile("test", source).unwrap();
        let compact = parsed.runfile.output(Defaults::Omit).render().unwrap();
        let restored = parse_runfile("compact", &compact).unwrap();
        assert_eq!(
            to_string(&parsed.runfile).unwrap(),
            to_string(&restored.runfile).unwrap()
        );
        assert!(compact.contains("OneElectron"));
        assert!(!compact.contains("linear_dependency_threshold"));
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "Configuration and portable round trips must preserve literal values and identical execution results exactly"
    )]
    fn schwarz_threshold_zero_survives_compaction_and_omission_restores_default() {
        let parsed = parse_runfile(
            "zero.toml",
            "[molecule]\n[basis]\nname = \"sto-3g\"\n[integrals]\nschwarz_threshold = 0\n",
        )
        .unwrap();
        assert!(parsed.integral_config.schwarz_threshold.value.is_none());
        let compact = parsed.runfile.output(Defaults::Omit).render().unwrap();
        assert!(compact.contains("schwarz_threshold = 0"));
        let restored = parse_runfile("compact.toml", &compact).unwrap();
        assert!(restored.integral_config.schwarz_threshold.value.is_none());

        let omitted = parse_runfile(
            "omitted.toml",
            "[molecule]\n[basis]\nname = \"sto-3g\"\n[integrals]\n",
        )
        .unwrap();
        assert_eq!(
            omitted
                .integral_config
                .schwarz_threshold
                .value
                .unwrap()
                .into_inner(),
            rustiq_core::config::DEFAULT_ERI_SCHWARZ_THRESHOLD
        );
        assert!(omitted.integral_config.schwarz_threshold.span.is_none());
    }
}
