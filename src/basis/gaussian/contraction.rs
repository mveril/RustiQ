use nalgebra::DVector;

#[derive(PartialEq, Debug, Clone)]
pub struct Contraction {
    /// angular momentum
    pub l: u8,

    /// spherical (pure) or cartesian
    pub pure: bool,

    /// contraction coefficients
    pub coeff: DVector<f64>,
}

impl Contraction {
    pub fn new(l: u8, pure: bool, coeff: Vec<f64>) -> Self {
        Self {
            l,
            pure,
            coeff: coeff.into(),
        }
    }

    /// Cartesian size of the orbital
    pub const fn cartesian_size(&self) -> usize {
        (self.l as usize + 1) * (self.l as usize + 2) / 2
    }

    /// Total size of the orbital
    pub const fn size(&self) -> usize {
        if self.pure {
            2 * (self.l as usize) + 1
        } else {
            self.cartesian_size()
        }
    }
}
