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
            item.as_table()?.get(key)
        })?;
        let span = item.span()?;
        Some((span.start, span.end.saturating_sub(span.start)).into())
    }

    /// Best-effort mapping of Nickel's contract label back to an explicitly
    /// written TOML value. Missing/defaulted values intentionally have no span.
    pub(crate) fn error_location(&self, error: &str) -> Option<(String, SourceSpan)> {
        const LOCATIONS: &[&[&str]] = &[
            &[
                "method",
                "hf",
                "orthogonalization",
                "linear_dependency_threshold",
            ],
            &["method", "hf", "convergence_threshold"],
            &["integrals", "schwarz_threshold"],
            &["method", "hf", "max_iterations"],
            &["method", "hf", "diis", "max_history"],
            &["method", "hf", "diis", "enabled"],
            &["method", "hf", "guess", "perturbation", "distribution"],
            &["method", "hf", "guess", "perturbation", "std_dev"],
            &["method", "hf", "guess", "perturbation", "mean"],
            &["method", "hf", "guess", "perturbation", "seed"],
            &["method", "hf", "guess", "perturbation", "min"],
            &["method", "hf", "guess", "perturbation", "max"],
            &["method", "hf", "guess", "std_dev"],
            &["method", "hf", "guess", "mean"],
            &["method", "hf", "guess", "seed"],
            &["method", "hf", "guess", "min"],
            &["method", "hf", "guess", "max"],
            &["method", "hf", "guess", "type"],
            &["method", "hf", "guess", "perturbation"],
            &["method", "hf", "guess"],
            &["method", "hf", "method"],
            &["method", "mp2", "frozen_orbitals"],
            &["method", "mp2", "memory_limit"],
            &["molecule", "multiplicity"],
            &["molecule", "geometry"],
            &["molecule", "charge"],
            &["molecule", "units"],
            &["basis", "name"],
            &["cache", "enabled"],
            &["output", "scf"],
        ];

        LOCATIONS
            .iter()
            .filter(|path| {
                let leaf = path.last().expect("configuration paths are non-empty");
                error.contains(leaf)
            })
            .find_map(|path| self.span(path).map(|span| (path.join("."), span)))
            .or_else(|| find_labeled_key(self.document.as_item(), &mut Vec::new(), error))
    }
}

fn find_labeled_key(
    item: &toml_edit::Item,
    parent: &mut Vec<String>,
    error: &str,
) -> Option<(String, SourceSpan)> {
    let table = item.as_table_like()?;
    for (key_name, value) in table.iter() {
        parent.push(key_name.to_owned());
        if error.contains(&format!("`{key_name}`")) {
            if let Some(span) = value.span() {
                return Some((parent.join("."), (span.start, span.end - span.start).into()));
            }
        }
        if let Some(found) = find_labeled_key(value, parent, error) {
            return Some(found);
        }
        parent.pop();
    }
    None
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
        let (path, _) = map
            .error_location("contract broken by the value of `frozen_orbitals`")
            .unwrap();
        assert_eq!(path, "method.mp2.frozen_orbitals");
    }
}
