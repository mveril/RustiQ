//! Source locations from Nickel's TOML parser. No configuration interpretation.
use miette::SourceSpan;
use nickel_lang_core::{
    eval::value::{Container, NickelValue},
    position::PosTable,
};
use std::collections::BTreeMap;

pub(crate) struct TomlSourceMap {
    locations: BTreeMap<String, SourceSpan>,
}

impl TomlSourceMap {
    #[cfg(test)]
    pub(crate) fn parse(source: &str) -> Result<Self, String> {
        super::nickel::source_locations(source).map_err(|errors| {
            errors
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join("\n")
        })
    }

    pub(crate) fn from_value(value: &NickelValue, positions: &PosTable) -> Self {
        fn visit(
            value: &NickelValue,
            positions: &PosTable,
            path: &mut Vec<String>,
            locations: &mut BTreeMap<String, SourceSpan>,
        ) {
            if let Some(span) = positions.get(value.pos_idx()).into_opt() {
                let start = span.start.to_usize();
                locations.insert(path.join("."), (start, span.end.to_usize() - start).into());
            }
            if let Some(record) = value.as_record().and_then(Container::into_opt) {
                for (name, field) in &record.fields {
                    if let Some(value) = &field.value {
                        path.push(name.label().to_owned());
                        visit(value, positions, path, locations);
                        path.pop();
                    }
                }
            }
        }
        let mut locations = BTreeMap::new();
        visit(value, positions, &mut Vec::new(), &mut locations);
        Self { locations }
    }

    pub(crate) fn span(&self, path: &[&str]) -> Option<SourceSpan> {
        self.locations.get(&path.join(".")).copied()
    }

    pub(crate) fn error_location(
        &self,
        path: Option<&str>,
        error: &str,
    ) -> (Option<String>, Option<SourceSpan>) {
        if let Some(path) = path {
            return (Some(path.to_owned()), self.locations.get(path).copied());
        }
        let mut matches = self.locations.iter().filter(|(path, _)| {
            let leaf = path.rsplit('.').next().unwrap_or(path);
            !leaf.is_empty() && error.contains(&format!("`{leaf}`"))
        });
        match (matches.next(), matches.next()) {
            (Some((path, span)), None) => (Some(path.clone()), Some(*span)),
            _ => (None, None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TomlSourceMap;

    #[test]
    fn maps_explicit_field_locations_and_nickel_contract_messages() {
        let source = "[basis]\nname = \"sto-3g\"\n[method.mp2]\nfrozen_orbitals = \"one\"\n";
        let map = TomlSourceMap::parse(source).unwrap();
        let span = map.span(&["method", "mp2", "frozen_orbitals"]).unwrap();
        let start = span.offset();
        assert_eq!(source.get(start..start + span.len()).unwrap(), "\"one\"");
        let (path, span) =
            map.error_location(None, "contract broken by the value of `frozen_orbitals`");
        assert_eq!(path.as_deref(), Some("method.mp2.frozen_orbitals"));
        assert!(span.is_some());
    }

    #[test]
    fn nested_inline_tables_retain_exact_value_spans() {
        let source = "method = { hf = { max_iterations = 0, diis = { max_history = 8 } } }\n";
        let map = TomlSourceMap::parse(source).unwrap();
        for (path, expected) in [
            (vec!["method", "hf", "max_iterations"], "0"),
            (vec!["method", "hf", "diis", "max_history"], "8"),
        ] {
            let span = map.span(&path).unwrap();
            assert_eq!(
                source
                    .get(span.offset()..span.offset() + span.len())
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn repeated_leaf_names_require_an_exact_path() {
        let source = "method = { hf = { guess = { distribution = 'Normal', mean = 1, seed = 2, min = 3, max = 4, perturbation = { distribution = 'Uniform', mean = 5, seed = 6, min = 7, max = 8 } } } }";
        let map = TomlSourceMap::parse(source).unwrap();
        for leaf in ["distribution", "mean", "seed", "min", "max"] {
            let message = format!("contract broken by the value of `{leaf}`");
            assert_eq!(map.error_location(None, &message), (None, None));
            for parent in ["method.hf.guess", "method.hf.guess.perturbation"] {
                let path = format!("{parent}.{leaf}");
                let (location, span) = map.error_location(Some(&path), &message);
                assert_eq!(location.as_deref(), Some(path.as_str()));
                assert_eq!(span, map.span(&path.split('.').collect::<Vec<_>>()));
                assert!(span.is_some());
            }
        }
    }

    #[test]
    fn an_omitted_known_path_never_falls_back_to_another_explicit_field() {
        let map = TomlSourceMap::parse("method = { hf = { guess = { seed = 42 } } }").unwrap();
        let path = "method.hf.guess.perturbation.seed";
        let (location, span) =
            map.error_location(Some(path), "contract broken by the value of `seed`");
        assert_eq!(location.as_deref(), Some(path));
        assert!(span.is_none());
        assert_eq!(map.error_location(None, "seed is invalid"), (None, None));
    }
}
