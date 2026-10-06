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
    super::nickel::export_toml(&json)
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
        let source = to_string(&item)?;
        let resolved = super::nickel::resolve_toml(&source).map_err(|errors| {
            RenderError(
                errors
                    .into_iter()
                    .map(|error| error.message)
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        })?;
        let mut paths = Vec::new();
        collect_fields(&item, &mut Vec::new(), &mut paths);
        for path in paths {
            // Preserve explicitly requested calculation sections in canonical output.
            if path == ["method"] || path == ["method", "hf"] {
                continue;
            }
            let mut candidate = item.clone();
            if !remove_field(&mut candidate, &path) {
                continue;
            }
            let source = to_string(&candidate)?;
            // Nickel owns defaults; serialization never supplies validation or defaults.
            if super::nickel::resolve_toml(&source).is_ok_and(|candidate| candidate == resolved) {
                remove_field(&mut item, &path);
            }
        }
        to_string(&item)
    }
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
    fn output_context_controls_defaults_without_changing_the_model() {
        let source = "[molecule]\n[basis]\nname = \"sto-3g\"\n[method.hf]\n[method.mp2]\n";
        let parsed = parse_runfile("test", source).unwrap();
        let full = parsed.runfile.output(Defaults::Include).render().unwrap();
        let compact = parsed.runfile.output(Defaults::Omit).render().unwrap();
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
