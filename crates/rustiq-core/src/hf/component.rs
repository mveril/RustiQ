use super::uhf::Spin;

/// A shared restricted component or separate unrestricted spin components.
#[derive(Debug, Clone, Copy)]
pub enum HfComponent<T> {
    Rhf(T),
    Uhf(Spin<T>),
}

impl<T> HfComponent<T> {
    /// Returns whether this component contains restricted Hartree-Fock data.
    pub const fn is_rhf(&self) -> bool {
        matches!(self, Self::Rhf(_))
    }

    /// Returns whether this component contains unrestricted Hartree-Fock data.
    pub const fn is_uhf(&self) -> bool {
        matches!(self, Self::Uhf(_))
    }
}
