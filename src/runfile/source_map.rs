//! TOML syntax locations only. Nickel owns configuration defaults and validation.
use miette::SourceSpan;

pub(crate) struct TomlSourceMap {
    document: toml_edit::Document<String>,
}

impl TomlSourceMap {
    pub(crate) fn parse(source: &str) -> Result<Self, toml_edit::TomlError> {
        toml_edit::Document::parse(source.to_owned()).map(|document| Self { document })
    }

    pub(crate) fn span(&self, path: &[&str]) -> Option<SourceSpan> {
        let item = path.iter().try_fold(self.document.as_item(), |item, key| {
            item.as_table_like()?.get(key)
        })?;
        let span = item.span()?;
        Some((span.start, span.end.saturating_sub(span.start)).into())
    }

    /// A known Nickel path is authoritative, even when its value was omitted.
    /// Without a path, only a uniquely labeled explicit TOML key is usable.
    pub(crate) fn error_location(
        &self,
        path: Option<&str>,
        error: &str,
    ) -> (Option<String>, Option<SourceSpan>) {
        if let Some(path) = path {
            return (
                Some(path.to_owned()),
                self.span(&path.split('.').collect::<Vec<_>>()),
            );
        }
        let mut matches = Vec::new();
        find_labeled_keys(
            self.document.as_item(),
            &mut Vec::new(),
            error,
            &mut matches,
        );
        if matches.len() == 1 {
            let (path, span) = matches.remove(0);
            (Some(path), Some(span))
        } else {
            (None, None)
        }
    }
}

fn find_labeled_keys(
    item: &toml_edit::Item,
    parent: &mut Vec<String>,
    error: &str,
    matches: &mut Vec<(String, SourceSpan)>,
) {
    let Some(table) = item.as_table_like() else {
        return;
    };
    for (key_name, value) in table.iter() {
        parent.push(key_name.to_owned());
        if error.contains(&format!("`{key_name}`")) {
            if let Some(span) = value.span() {
                matches.push((parent.join("."), (span.start, span.end - span.start).into()));
            }
        }
        find_labeled_keys(value, parent, error, matches);
        parent.pop();
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
