use periodic_table::Element;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("Unable to parse atomic mass '{mass}': {source}")]
pub struct AtomicMassParseError {
    mass: &'static str,
    #[source]
    source: std::num::ParseFloatError,
}

pub(crate) trait ElementExt {
    fn atomic_mass_f64(&self) -> Result<f64, AtomicMassParseError>;
}

impl ElementExt for Element {
    fn atomic_mass_f64(&self) -> Result<f64, AtomicMassParseError> {
        let mass = self.atomic_mass;
        mass.split_once('(')
            .map(|(before, _)| before)
            .unwrap_or(mass)
            .trim_matches(['[', ']'])
            .parse::<f64>()
            .map_err(|source| AtomicMassParseError { mass, source })
    }
}

#[cfg(test)]
mod tests {
    use periodic_table::periodic_table;

    use super::ElementExt;

    #[test]
    fn test_atomic_mass_parses_for_all_classic_elements() {
        for element in periodic_table() {
            element
                .atomic_mass_f64()
                .unwrap_or_else(|err| panic!("{}: {}", element.symbol, err));
        }
    }
}
