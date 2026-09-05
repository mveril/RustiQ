use toml_spanner::{Arena, FromToml, Item, TableStyle, ToToml, ToTomlError};

use super::RunFile;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Defaults {
    Include,
    Omit,
}

pub(crate) struct TomlOutput<'a, T> {
    value: &'a T,
    defaults: Defaults,
}

impl RunFile {
    pub(crate) fn output(&self, defaults: Defaults) -> TomlOutput<'_, Self> {
        TomlOutput::new(self, defaults)
    }
}

impl<'a, T> TomlOutput<'a, T> {
    pub(crate) fn new(value: &'a T, defaults: Defaults) -> Self {
        Self { value, defaults }
    }
}

impl<T: ToToml + for<'de> FromToml<'de>> ToToml for TomlOutput<'_, T> {
    fn to_toml<'a>(&'a self, arena: &'a Arena) -> Result<Item<'a>, ToTomlError> {
        let original = self.value.to_toml(arena)?;
        if self.defaults == Defaults::Include {
            return Ok(original);
        }
        let mut item = original.clone_in(arena);
        let mut paths = Vec::new();
        collect_fields(&original, &mut Vec::new(), &mut paths);
        for path in paths {
            let scratch = Arena::new();
            let mut candidate = item.clone_in(&scratch);
            if !remove_field(&mut candidate, &path) {
                continue; // An earlier omission removed the containing table.
            }
            let source = toml_spanner::to_string(&candidate)?;
            // FromToml is the authority for defaults, including custom
            // expressions, nested sections, and required enum tags.
            if let Ok(restored) = toml_spanner::from_str::<T>(&source) {
                if restored.to_toml(&scratch)? == original {
                    remove_field(&mut item, &path);
                }
            }
        }
        Ok(item)
    }
}

// Visit whole fields before their children so a default section can disappear
// in one step. Arrays are kept intact unless the entire field can be omitted.
fn collect_fields(item: &Item<'_>, path: &mut Vec<String>, paths: &mut Vec<Vec<String>>) {
    if let Some(table) = item.as_table() {
        for (key, value) in table.iter() {
            path.push(key.as_str().to_owned());
            paths.push(path.clone());
            collect_fields(value, path, paths);
            path.pop();
        }
    }
}

fn remove_field(item: &mut Item<'_>, path: &[String]) -> bool {
    let Some((key, parents)) = path.split_last() else {
        return false;
    };
    let parent = parents
        .iter()
        .try_fold(item, |item, key| item.as_table_mut()?.get_mut(key));
    let Some(table) = parent.and_then(Item::as_table_mut) else {
        return false;
    };
    let removed = table.remove_entry(key).is_some();
    if table.is_empty() {
        table.set_style(TableStyle::Header);
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runfile::parser::parse_runfile;

    #[test]
    fn omission_uses_from_toml_defaults_without_a_default_trait_or_known_fields() {
        #[derive(toml_spanner::Toml)]
        #[toml(Toml, recoverable)]
        struct Config {
            required: u32,
            #[toml(default = 7)]
            retries: u32,
            nested: Nested,
            #[toml(default)]
            optional: Option<Nested>,
        }

        #[derive(toml_spanner::Toml)]
        #[toml(Toml)]
        struct Nested {
            #[toml(default = 3)]
            limit: u32,
        }

        let config = Config {
            required: 0,
            retries: 7,
            nested: Nested { limit: 3 },
            optional: Some(Nested { limit: 3 }),
        };
        let compact = toml_spanner::to_string(&TomlOutput::new(&config, Defaults::Omit)).unwrap();
        assert!(compact.contains("required = 0"));
        assert!(!compact.contains("retries"));
        assert!(!compact.contains("limit"));
        assert!(compact.contains("[nested]"));
        assert!(compact.contains("[optional]"));
        let restored: Config = toml_spanner::from_str(&compact).unwrap();
        assert_eq!(restored.retries, 7);
        assert_eq!(restored.nested.limit, 3);
        assert_eq!(restored.optional.unwrap().limit, 3);

        let config = Config {
            retries: 0,
            nested: Nested { limit: 9 },
            ..config
        };
        let compact = toml_spanner::to_string(&TomlOutput::new(&config, Defaults::Omit)).unwrap();
        assert!(compact.contains("retries = 0"));
        assert!(compact.contains("limit = 9"));
    }

    #[test]
    fn output_context_controls_defaults_without_changing_the_model() {
        let source = "[global]\nbasis = \"sto-3g\"\n[hf]\n[mp2]\n";
        let parsed = parse_runfile("test", source).unwrap();
        let full = toml_spanner::to_string(&parsed.runfile.output(Defaults::Include)).unwrap();
        let compact = toml_spanner::to_string(&parsed.runfile.output(Defaults::Omit)).unwrap();
        for field in [
            "charge",
            "multiplicity",
            "molecule_unit",
            "method",
            "max_iterations",
            "convergence_threshold",
            "linear_dependency_threshold",
            "diis",
            "guess",
            "frozen_orbitals",
        ] {
            assert!(full.contains(field), "missing {field}");
            assert!(
                parsed.formatted_toml.contains(field),
                "display missing {field}"
            );
            assert!(!compact.contains(field), "unexpected {field}");
        }
        assert!(compact.contains("[hf]"));
        assert!(compact.contains("[mp2]"));
        let restored = parse_runfile("compact", &compact).unwrap();
        assert_eq!(
            full,
            toml_spanner::to_string(&restored.runfile.output(Defaults::Include)).unwrap()
        );
        assert_eq!(
            full,
            toml_spanner::to_string(&parsed.runfile.output(Defaults::Include)).unwrap()
        );
    }

    #[test]
    fn compact_output_preserves_non_default_and_tagged_configuration() {
        let source = r#"
[global]
basis = "cc-pvdz"
[global.molecule]
charge = -1
multiplicity = 2
molecule_unit = "Bohr"
[hf]
method = "Uhf"
max_iterations = 42
diis = true
[hf.guess]
type = "OneElectron"
[hf.guess.perturbation]
distribution = "Normal"
mean = 0.0
std_dev = 0.01
[mp2]
frozen_orbitals = 1
"#;
        let parsed = parse_runfile("test", source).unwrap();
        let compact = toml_spanner::to_string(&parsed.runfile.output(Defaults::Omit)).unwrap();
        let restored = parse_runfile("compact", &compact).unwrap();
        assert_eq!(
            toml_spanner::to_string(&parsed.runfile).unwrap(),
            toml_spanner::to_string(&restored.runfile).unwrap()
        );
        assert!(compact.contains("OneElectron"));
        assert!(!compact.contains("linear_dependency_threshold"));
    }
}
