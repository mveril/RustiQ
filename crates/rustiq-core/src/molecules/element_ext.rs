use mendeleev::Element;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("Element {symbol} has a non-finite atomic weight")]
pub struct AtomicWeightError {
    symbol: &'static str,
}

pub(crate) trait ElementExt {
    fn atomic_weight_f64(&self) -> Result<f64, AtomicWeightError>;
}

impl ElementExt for Element {
    fn atomic_weight_f64(&self) -> Result<f64, AtomicWeightError> {
        let weight = f64::from(self.atomic_weight());
        if weight.is_finite() {
            Ok(weight)
        } else {
            Err(AtomicWeightError {
                symbol: self.symbol(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use mendeleev::Element;

    use super::ElementExt;

    #[test]
    fn test_atomic_weight_is_finite_for_all_elements() {
        for element in Element::iter() {
            element
                .atomic_weight_f64()
                .unwrap_or_else(|err| panic!("{}: {}", element.symbol(), err));
        }
    }
}
